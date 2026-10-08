# Frontend 1:1 Gaps

## Source44：Hero 共享页面及输入接线通过有限验证；新版完整包仍待交付（2026-10-08）

Page 与 Shell 已接入共享 Hero Host：隐藏根先完成布局，Prepared 后同步卸载旧 DOM，再启用控制并等待实际 matching Ready；渲染期只读投影，事件和原发送前 claim 重核来源、租约、sink、布局及500ms帧健康。鼠标、键盘、悬停与移动菜单接原输入仲裁；旧 pointer/cleanup 不得跨场景或接管新手势。同内容完整快照保留合法拖拽，新来源或控制变化清除旧排队输入和反馈。继续复用原唯一操作台账、精确ACK与后续完整快照屏障及 Unknown custody。以上为代码接线与有限回归，不是 live World/Ready 或玩家验收。

实际隔离 Rust02 Runtime26/26＋portable66/66，共92次通过，新增5个测试函数；本轮未重新执行 Native。WebGL2 shared 与 WebGPU 两项静态 cargo check exit0。实际 Web03 原脚本185/185 TAP通过（155NPC购买＋29surface＋1Stage5脚本），Stage5另有494逻辑组＝原477＋新增17；TypeScript5.9.3 no-emit/nonincremental exit0、stdout/stderr均0B。旧测试业务断言继续执行；修复夹具依赖不表示整个旧runner逐字不变。

917项隔离 Rust 输入、17130项 Web 输入及10个本轮源码在各自有限区间冻结，最终全量重核匹配；不合并声称当前整checkout或后端全部通过。四次最终 Cargo 均严格串行且原PolicyB completed/exited/disposed，实际C最低199272189952B、freshness保守上界最大79ms，原50GiB/2000ms门槛不变。原始64份最终/历史输出与配置归档；Rust01新夹具 spawned 状态不一致的一项失败、Web01/02遗漏新增Hero闭包依赖的失败完整保留，仅修夹具后通过。

下一步复核默认共享入口与九类任务动作整链，再交付源码匹配的renderer/Core/EXE/Web完整包。Matrix11仍103 shared＋206 legacy＋8 common；32.5%仅共享分类覆盖，97.5%仅含旧实现的功能记录覆盖，overallPercentage=null、Candidate100=false、goal active。最新完整Web仍Source25 Next08/Thin08，不含Source26–44；真实登录/任务/战斗/保存/重登、移动真机及最终人类验收not-run，继续代码且暂不操作界面，尚无可信可玩日期。用户tsconfig、外来制作加工/Cargo/simulation/文档变更保留且排除本轮提交。

证据：[实际有限结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-hero-source44-finite-result01.json)、[64份原始文件](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-hero-source44-raw-evidence01.json)、[独立实际复核](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-hero-source44-actual-review01.json)、[Rust首次夹具失败](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-hero-source44-rust01-historical-failure01.json)、[Web首次失败](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-hero-source44-web01-historical-failure01.json)、[Web第二次失败](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-hero-source44-web02-historical-failure01.json)、[Source43实际推送](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-hero-source43-publication01.json)。Source43已推送f359d5b5d0cebd964eadd99b8a46d1ca52ecbd1f；本轮提交推送随后另存实际记录，下方保留历史。

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

## Source30：Web 购买回执实际接线通过代码回归，Native 整体应用有限回归通过（2026-10-07）

Web 原始 owner frame 在普通日期/数值 decoder 前进入永久 Core host；仅明确购买手势走 Quote→Reserve→Enter→单次 socket send，重连只查询原 operation。完整经济来源同步替换钱包、背包/装备、邮件及 Stage5，并绕过 movement-only 和 packetRefresh 旧投影；随后才提供 Applied。修正真实非空英雄装备 WorldItem 格式、钱包相同值的目录 custody、过期/重入 Enter 保留 Unknown、fraction/exponent 舍入边界。wire Cargo.toml/src 已纳入实际 Core/PUI 构建指纹。

Root 实际原 NPC 脚本 128/128（原106＋新增22，含两个纯模块严格 no-emit 类型检查），构建指纹2/2，共130次最终选定 Node 测试；144＋64输入前后复核零漂移。本机原 Web/root依赖已缺失，仅复用已安装官方 TypeScript5.9.2 的121普通文件用于有限源代码测试，不等于项目原工具链、Page全量类型检查或生产构建。首轮126/128失败保留并修正；一次配置仍引用旧快照而被前置哈希拒绝，修正配置后通过。

Native 已新增永久购买 client、整体预解码经济 bundle、独占 World 应用及有类型的 Applied 回传；修正 same revision 正常移动/时钟误判、稀疏14装备槽、公开 Hero/city/Character 领域和 mail reserve 展开容量。877 Rust 输入已冻结，Root 实际 Native14/14＋Runtime30/30＝44个唯一 Rust 测试，加 Node130 共174次最终选定执行，新增68个 distinct（44 Rust＋22 NPC＋2指纹）。两个原 CargoGuard 实际 GetVolume C≥50GiB，sampleEnd→childStarted 实际毫秒截断上界分别78.5672ms/75.501ms<2000ms，PolicyB Completed/Exited/Dispose；877 Rust 输入前后零漂移。实际 Native gateway receiver/source projector/Applied pump、只读 Quote 保持原 UI proof 和最终 Purchase sender 仍未接线；Hero/Social、邮件内容与 tooltip 的同源投影也须在该入口落实。

大整数 UID/i64 日期 Web 投影仍 fail closed，英雄可视经验/maxXP 仍有旧包缓存来源；下一步继续无损快照与实际 Native 接线，再恢复匹配依赖、重建两端候选包。当前实际包仍 Source25 Next08/Thin08，不含 Source26–30。Matrix11保持309/317≈97.5%有界代码记录，不是可玩或全项目完成率；overallPercentage=null、Candidate100=false、goal active。用户“继续代码，暂不操作界面”保持有效，登录→战斗→保存→重登、资源/WASM、移动真机及玩家验收均 not-run，尚无可承诺试玩日期。Source29已实际提交推送并核验 a04462cb48e1d22acff1a2c532ba8faaabb5e474，本轮发布另以实际Git结果为准。

证据：[Source30完整有限结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/npc-purchase-source30-finite-result01.json)、[Source30 Web实际有限结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/npc-purchase-source30-web-finite-result01.json)、[Source29发布](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/npc-purchase-source29-publication01.json)。以下历史正文保留。


## Source29：严格购买 wire、永久 Core host 与 Web ABI 已通过，实际客户端应用继续接线（2026-10-07）

新增独立轻量 client-wire，外部 u64 采用 canonical decimal string，保留 UID0 与 MAX selector；递归拒 duplicate/escaped duplicate、unknown 和数值型 ID。control request ID 与完整 actor/scope/sequence/intent 分开关联，producer/snapshot/authority 必须同对；terminal revision 不能取 MAX，快照 JSON 本身不证明客户端完整应用。

共享 Core host 按 Actor 保留 ledger，跨 UI、同 producer 重连及更换角色不清原 operation、不换 ID、不自动重发；容量与 allocator 耗尽不驱逐历史。Begin/连接更换先撤回 availability，再等 fresh receipt 与完整 applied witness。Web ABI 独立保留 control/ticket，严格 raw frame 匹配后才观察结果；document 永久 facade 将 token 同原 producer/physical socket 绑定，拒 getter/toJSON 和反射重入越过最终 generation fence，旧回调不能借新连接权限。

Gateway actual reader 重新严格解析原 message 后进入专用 serial 队列，实际 authenticated/verified 当前身份、opt-in 和真实 owner 能力共同准入；不走旧 BuyItem fallback。同 owner turn 的 snapshot 与 authority 一起输出，2MiB projection 失败仅保留精确关联的真实 PostCommit receipt。Gold/Pearl goods 增加 purchaseItemIndex 字符串，在 JS Number 转换前保留完整 selector。

Root 最终 Rust02 实际 Gateway13＋wire16＋Core187＋ABI13＋旧Gold16＝245个唯一具名测试；原 NPC 脚本106/106（新增12），严格非增量 TSC exit0/无诊断，共351次最终选定执行，新增48 Rust＋12 Web＝60项 distinct。897 Rust 与27915 Node 普通输入全部独立核验、零漂移；原888 Rust 中11改/877保护，9个快照输入新增，零移除。9次实际原 CargoGuard、45 nonce files 全部 actual GetVolume C≥50GiB、sampleEnd→childStart 上界≤85.5647ms<2000ms、PolicyB Exited/Dispose。

首轮 Gateway workspace roots 编译101/0测试保留，最小修复仅将 client-wire 加到根 exclude；历史 wire/Core/ABI 216次不累加进最终245。结果01派生 freshness 曾误用 probeEnd 差值；独立复核后结果02改为实际 sampleEnd 到 childStarted 的截断毫秒界限，原收据、源码、计数不变，不重复测试。

下一项是 Native/Web actual receiver/dispatcher 与完整经济 bundle。Native 实际用 WS JSON，当前 Applied 只证明入队，多个模型各自 coalesce；须整体预解码、主线程完整应用后回传 witness。Web 新 owner frame 须在普通 decoder 前保留 raw string，完整应用同 revision 的个人经济状态并绕过 movement-only 快路径；inventory/equipment readiness、incoming UID、tick 或入队都不能代替完整应用。

匹配重建前须将 client-wire manifests/src 纳入实际 Core/PUI builder fingerprint 及原 fixture copy。当前已构建包仍是 Source25 Next08/Thin08，不含 Source26–29；原 full11 包回归 node01 本轮只准备未运行，不改旧 manifest/资产伪造新包。Matrix11 原字节保持103 shared/206 legacy/0 raw open/8 common，309/317≈97.5%仅是有界代码记录，不是可玩或全项目百分比；overall percentage=null、Candidate100=false、goal active。用户“继续代码，暂不操作界面”持续有效，登录→战斗→保存→重登、资源/WASM与移动/frontend验收均 not-run，当前不承诺可玩日期。

Source28 已实际提交推送并核验远端 fdd3abc37d3ab6df45b428013dda85501d7404c5；本轮承接其事后 publication。Source29 Git 发布由 Root 实际操作后另记，不以测试或文档代替推送证据。

证据：[本轮实际有限结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/npc-purchase-source29-finite-result02.json)、[独立结果复核](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/npc-purchase-source29-independent-review01.json)、[Source28 实际发布](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/npc-purchase-source28-publication01.json)。以下 Source28 及更早正文完整保留为历史。


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

证据：[本轮实际有限结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/npc-purchase-source28-finite-result01.json)、[独立有限结果复核](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/npc-purchase-source28-independent-review01.json)、[Source27 实际发布](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/npc-purchase-source27-publication01.json)。


## Source27：本地购买存档与恢复通过，网关及两端回执继续接线（2026-10-07）

新增私有 Actor journal，把原 request scope/sequence/full intent、实际 Gold/Pearls、Trade/BuyBack/Used 结果及权威 revision 与完整 character checkpoint 放进既有 account source 的同一次 transaction。当前活动 Session 提供 producer enrollment、实际 catalog proof、原 ID 只读 query 和单次 durable purchase；精确 duplicate 不再次写档、扣款、交付或删除 stock，terminal rejection 保留。journal 不进入普通 WorldSnapshot，旧存档的 optional 缺失保持兼容。

购买先在同一个过期维护 clone 上校验 proof/currency/source，再规划钱包、库存、所选 stock 和输出；完整 commit 后才发布 live 状态。mail、mentor、relationship、经济标记、Pearls、婚戒装备及 XP/level/roster 的合并结果同步发布。File rename 结果不明时保留 Unknown 并冻结旧 cache；已确认 PostCommit 的精确 receipt 不被后处理错误或 panic 吞掉。失败 LogOut/Disconnect save 仍先退役购买 authority，同 Session restore 不允许回滚旧 actor/history。

本轮真实重登夹具发现已保存的空 bag/storage 会被旧 demo 初始化补种，现以 committed revision 或已有 journal 保护整个 checkpoint，保留 revision0/无 journal 的旧初始化。首轮46/48与第二轮49/50原日志保留：另一处是夹具只改 sequence、未改实际 request；新增 legacy 期望则明确保留既有 TownTeleport Bag2/slot0 UID0→40 规范化，仅更正该独立期望，完整 checkpoint、两次 load 与 File 不变断言均保持。

Root 最终 Rust03 实际 Simulation50＋旧Gold15＋Pearl6＋Save59＋Demo1＝131，再加既有 Gateway Gold owner16＝147次成功执行/146个唯一测试（Pearl一项重复选中）；新增 journal18＋Session/File19＝37项 distinct 全部具名执行。全部6次成功调用使用最终 Rust03，2次历史失败不累加其部分通过。884唯一普通输入0漂移，原881中7改/874保护、新增3；8次串行原 CargoGuard 均为 actual GetVolume C≥50GiB、fresh≤2000ms、PolicyB exited/Dispose。5份 Rust02 回归配置只准备未执行，不计测试。

这里完成的是本地 Session/File 边界。下一项接真实 Gateway owner/Hosted/RPC 的能力与原 ID query、mutation 单次发送且 Unknown 不 fallback，再接 strict wire/ABI、Native/Web 实际 dispatcher/receiver 和含权威经济 revision 的完整 applied snapshot。当前个人 NPC checkpoint 不证明 shared-zone Used stock 原子所有权；当前新 Session/独立 File 检查也不代表另一个 OS process 的冷启动或并发 owner 验收，继续单列验证。客户端 opt-in 不能当服务端已接受能力。

已构建包仍是 Source25 的 Next08/Thin08，不含 Source26/27，待完整接线后重建匹配组合。用户“继续代码，暂不操作界面”持续有效；实际资源/WASM 初始化、登录→战斗→保存→重登、移动真机和最终 frontend 验收未运行。Matrix11 原字节保持103 shared/206 legacy/0 raw open/8 common；317条是有界记录，不是全项目分母，overall percentage=null、Candidate100=false、goal active。

Source26 已实际提交推送并核验远端4cbba7a193e5283b8f7d92ee2be8416262cd8d68；本轮承接其事后 publication 收据。Source27 的提交推送状态另由 Root 实际操作核验，文档和测试本身不证明发布。证据：[本轮实际有限结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/npc-purchase-source27-finite-result01.json)、[Source26 实际发布](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/npc-purchase-source26-publication01.json)。以下 Source26 及更早正文完整保留为历史，本轮状态以上述 Source27 为准。


## Source26：购买本地结果与共享未决状态通过有限回归，完整回执链继续（2026-10-07）

服务端新增全NPC购买的直接本地处理结果，实际标明Gold/Pearls、Trade/BuyBack/Used、扣款、实际准入数量及incoming UID。Gold回购/二手路径先完整规划库存、所选stock与输出，再提交钱包/库存/NPC；零数量有限stock提交前拒绝。旧Gold严格接口、价格、数量clamp、整条resale stock删除与合法旧SomeDura载体保持，UID0允许。stock expiry预处理仍是独立维护，不承诺失败回滚过期维护、复制全部额外属性或durable提交。

共享Core新增独立于transport的Actor绑定单笔经济状态：保留original request ID/full intent与checked sequence；当前producer完整baseline＋checked revision门槛，精确terminal receipt与当前已应用完整snapshot两序都具备才结算。Unknown、旧producer、错Actor、错tuple及partial snapshot不清未决；同Actor重连仅给原ID只读恢复信息，foreign Actor不能覆盖旧ledger。此纯模块尚未接ABI、Native/Web真实dispatcher、网络协议或持久journal，不能把定义或测试当成前端购买恢复已完成。

Root实际Core173＋Simulation新12/旧Gold15/Pearl6＋Gateway旧Gold owner16＝222次选定成功执行，新增Core21＋Simulation12＝33项distinct声明均具名执行。Core173在Rust02运行，之后仅Simulation新测试夹具改变、Core有效输入完全未变而限定承接；其余49在最终Rust04 fresh。Simulation01编译失败/0测试与02的11通过/1失败原日志保留；分别只纠正测试数量类型u16和错误非零UID假设，完整carrier比较及真实交付身份断言保持。881唯一regular输入0漂移，原878中7改/871保护、新增3；七次串行原CargoGuard actual GetVolume C≥50GiB、fresh≤2000ms、PolicyB退出/Dispose均核对。这里不是全workspace或玩家验收。

下一项继续全货币/source的durable journal＋完整character checkpoint＋receipt/revision原子提交，再贯通owner/remote版本化结果与只读查询、实际capability接受、严格wire/ABI及两端receiver/full snapshot。Matrix11逐字保持103shared/206legacy/0raw open/8common；317条有界记录不是全项目分母，overall percentage仍为null、Candidate100=false、goal active。不能把原始缺口分类关闭当成已经稳定可玩。

已构建并推送核验的包仍是Source25提交7b1afe2706a4eec037f085d1f599d2ea987f031f的Next08/Thin08；本轮没有重建生产组合，该包不含Source26购买代码。用户“继续代码，暂不操作界面”持续有效，实际资源/WASM初始化、登录→战斗→保存→重登、移动真机与最终frontend验收未运行。Source26 Git发布由Root实际提交推送后另报，文档/收据本身不证明发布。

证据：[本轮有限结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/npc-purchase-source26-finite-result01.json)、[Source25实际发布](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/ranking-source25-publication01.json)。以下Source25及更早正文保留为历史，本轮状态以上述Source26为准。


## Source25：排名查看代码与候选包已完成，实际游玩待验收（2026-10-07）

排名查看已接入共享Rust准入、可选PUI严格ABI、Native真实排名身份及Web实际发送入口；返回PlayerInspect使用真实14装备位置与raw carrier/tooltip，窗口只读。后端采用严格离线快照和本地在线当前owner投影，登录、保存失败、退出及旧连接退役均有具名回归；远端owner仍Unknown、Observe禁用、旧wire无request nonce，不宣称跨服完整能力。

实际客户端选定Rust240/240，Backend80/80（新增12＋56＝68项distinct Rust均执行），原11个Web脚本220/220；Stage5内部440组只计一个文件级测试。严格非增量类型检查通过。Simulation63、Gateway unit5按已执行且未变有效输入限定承接，Gateway chain12在最终Backend06 fresh；历史失败及原断言保留，不称全workspace或全部320在最后快照重跑。

Core/PUI、Windows EXE、Gateway、三renderer、Next08和Thin08均已实际构建通过并独立静态复核，未启动。PUI WASM261575 B，原strict262144 B门槛余569 B。Next08 fresh strictTypeScript20.9s/13静态页、19007files/610069210 B；Thin08 63203冻结输入，7301files/776dirs含根/0links/373130249 B，原377487360 B cap余4357111 B。11 runtime副本、231实际JSON source/output pairs与44 warning完整多重集均核对；JSON仅按相同字节限定承接历史token结论，无新parser。包位于apps/web/.mir2-thin-client-web-windows-catchup-20261007-08。

Matrix11仅F11.RANKING.INSPECT由open转为legacy/sourceCandidate，103shared/206legacy/0open/8common；其余316整row、317ordered IDs、native/originalAudit和raw历史保持。358关联/226唯一声明/每row去重357是具名检查映射，不能当执行或验收数量。317不是全项目分母，overall percentage=null、Candidate100=false、goal active。

用户“继续代码，暂不操作界面”继续有效；实际资源/WASM初始化、标准登录→角色→移动换图→任务/战斗/掉落→保存退出重登、移动真机及玩家验收全部not-run。Mount/Pet/Gate等遗漏资源需要配置不可变origin并通过release:doctor/真实smoke，静态包不证明在线可玩。下一代码项是跨Gold/Pearl/BuyBack/Used关联购买回执及恢复去重，entered/flushed/unknown不能当成功或自动释放；之后继续宽队列和未穷尽变体。

Source24已实际提交推送并核验远端6562c474a67cf1c377a600ae254997be744879b4。本段记录Source25源码和构建结果；本轮Git发布状态由Root实际commit、push及远端核验单独报告。以下Source24及更旧状态保留为历史，旧pending/unhooked句不是当前结果。

证据：[矩阵11](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/feature-matrix11.json)、[当前静态组合构建](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/ranking-source25-combined-build-result01.json)、[客户端有限结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/ranking-source25-client-finite-result01.json)、[后端有限结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/ranking-source25-backend-finite-result01.json)、[Web有限结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/ranking-source25-web-finite-result01.json)。


## Source24：Pearl 源码候选闭合，玩家验收待运行（2026-10-07）

仅 `F09.NPC.PEARL` 由open转为legacy/sourceCandidate closed：完整raw Pearl目录与已知钱包接入实际GameShop confirm/double-click、Page共享Core报价/准入与单用opaque claim紧邻socket.send；UID0保留，数量/价格由共同Rust planner与f32单价oracle决定，optional PUI Pearl ABI2拒绝缺失或失效事实。Gold授权继续独立；共同规则不将整个DOM painter升为shared，raw BuyItem三字段未变。

实际客户端Rust148＋70＋24＝242/242，17项新增distinct；后端Gold54＋Pearl6＝60/60，4项新增distinct。Pearl01错误filter实际0 tests仅证明compile，不计60，旧记录保留。Web原11脚本220/220与独立NPC购买94/94分开；Stage5内部392组只计220中的一个file-level测试。strict非增量TSC02实际退出0，其后仅排除的MJS夹具改变、有效TS输入未变而限定承接；Next07 fresh strict TypeScript14.4s/13静态页。以上是有限CPU/源夹具与静态构建，关联数不等于执行数。

Core/PUI实际WASM252590 / 262065 B、JS24393 / 23779 B，原WASM<262144 B与JS≤204800 B预算保持，PUI距门槛仅79 B。Native实际104700416 B/SHA256 `2d36a8a24120f2de8ccbb80221b7994a31c784333d1dfc515c71237c909c2224`，Gateway105727488 B/SHA256 `b9302153eead156227169fa3e5f94d6024145f590c5fa75d124d66df481ef347`；三renderer fresh构建退出0，版本仍 `bevy-8e38472ba5cf5ef3`。均未启动，renderer immutable release仍为原ignored生成资源。

Next07实际19007文件/308目录含根/608768029 B，61 NFT/122259 raw引用→37094 canonical（37092 regular＋2 declared junction），0missing/private且不遍历junction。Thin07实际退出0、23988ms，63195 canonical输入/7301文件/776目录含根/373096153 B/0links；原cap377487360 B余4391207 B。独立source-only静态bundle审查接受、0 blocker；44 warning完整多重集及231 JSON actual source/output pairs与Source23相同，仅限定承接historical token结论，未新跑parser/lexer。

后端先选择真实合法空槽，库存/NPC所选stock/输出转换先staging，最后连续写钱包/库存/NPC；失败不扣款、不失目标Used stock，保留旧SomeDura/legacy metadata/UID allocator。既有expiry预处理是独立维护，不是rollback；不承诺全部stats/sockets复制或catalog UID成为delivery UID。共同经济wire仍无correlated ACK，entered/flushed/unknown保持未决，Native无限stock同UID/count pending；完整structured catalog上限2MiB与既有complete authority1MiB分开，超后者fail closed。

矩阵10为103 shared/205 legacy/1 open/8 common limitation；只改Pearl一行，其余316整row、317 ordered IDs、全部native/originalAudit/rawCounts及玩家not-run保持。当前有限字段实际332关联/200唯一声明/按row去重331，path＋name/declaration身份；F01.safe-key.open原duplicate保留，Pearl旧声明转historical，当前仅21个NPC-buy literal Pearl声明（94 actual包含这些）。唯一remainingConfirmedRawGapId是 `F11.RANKING.INSPECT`，317不是完整验收分母，无overall percentage、Candidate100=false、goal active。

Source23已实际push并核验远端 `c63136131bf9bc65e482188920c1c2a37730aeba`；Source24 commit/push仍pending，由Root实际执行后报告。下一项Source25 Ranking Inspect与PUI体积优化，再跨货币correlated经济回执及宽队列；新增ranking_inspect.rs仍unhooked/uncompiled，不计done。用户“继续代码，暂不操作界面”持续有效，真实UI/HTTP/socket/网络游戏/WASM API或实例、登录/游戏/保存重登、mobile与最终frontend玩家验收均未运行。

证据：[矩阵10](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/feature-matrix10.json)、[当前证据10](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/current-evidence10.json)、[客户端有限结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/pearl-source24-client-finite02.json)、[Web有限结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/pearl-source24-web-finite01.json)、[后端有限结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/pearl-source24-backend-finite01.json)、[动作链复核](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/pearl-source24-action-chain-review01.json)、[组合构建](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/pearl-source24-combined-build-result01.json)、[Source23实际发布](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/fishing-source23-publication02.json)。以下全部旧正文（包括Progress77后端有限段）逐字保留为历史，旧pending与“未修复”仅表示当时状态，当前以上述Source24为准。

## Source23：世界钓鱼源码候选闭合，玩家验收待运行（2026-10-07）

仅`F02.world.fishing-click`由open转为legacy/sourceCandidate closed。实际Page/Shell主鼠标或stage touch Walk接入共享Rust钓鱼几何、转向/一秒cast clock及自有动画状态机；raw fishing/有符号i16 transform、三邻格阻挡与water均取typed事实，unknown不补standing或0。useLayoutEffect提交的只读pose绑定独立epoch/incarnation/continuity和≤250ms新鲜度；真实rod UID0、layout、owner/socket/session/map/Core与物理指针共同约束授权。terminal先burn再callback，第二指针/blur/旧cleanup与重入不得复活旧手势；unknown必须fresh down，合法cooldown/nonstanding/nonwater None可保留held retry。最后不透明单用proof紧邻真实socket.send，发送unknown保留cast clock且不自动重试；这不证明真实运行或经济ACK。共同规则不将整个DOM world painter升为shared。

原11个Node脚本Node05实际218/218、0失败/0跳过；Stage5原318＋新增39＝357内部组只计218中的一个file-level测试，不能相加。strict code-only非增量TSC05与Next生成metadata后的TSC06均退出0。原具名夹具保持，真实Fishing取消依赖及空refs已接入；Combat保留Auth/Repair/Bag proof，新增精确Fishing拒绝门令socket索引+4→+5、尾部3→4；Storage原Equipment位置-1→-2并追加Fishing -1，Social保留，Quest仅增加空queued ref。字符串label与lexical `test(`统计口径已在有限证据06更正，均不等于展开执行数量。原三次体积失败、Bag63通过/1失败、Node04实际130通过/22失败/152 reported（Storage早停）全部保留。

Backend实际7 jobs/242次通过、6项新增distinct；最终PUI Rust07 fresh64通过/0失败含新增6项手工decoder oracle，旧Fish/Tooltip/Bag测试保持。历史Rust04五jobs266通过＋3项原ignored（Core141/PUI58/default44/NativeFish6/Runtime17），PUI58由当前64替代；NativeInput133及其余历史配置仅按未变有效输入限定承接，不称全Rust07重跑、完整suite或总distinct。

Core/PUI actual build05通过，WASM/JS分别252590/24393 B与259415/22264 B，原strict WASM<262144 B及JS≤204800 B保持；PUI距WASM门槛2729 B。三renderer实际通过、版本`bevy-8e38472ba5cf5ef3`。Native104656384 B/SHA256`2c473cae9153b8af53e4dddb5fb700d214f5cf1ece780cad32dd4eaa5a5563ee`及Gateway105714688 B/SHA256`ac512de8899da64c0c82c4273f08c4954dce2a03780518e716774221f862f544`已实际构建且未启动；renderer immutable release仍为既有ignored生成资源。

Next06实际退出0、80826ms，strict TypeScript11.7s/13静态页；27439声明输入、19006文件/308目录含根/599650114 B，61 NFT/37094引用（37092 regular＋2声明junction），0missing/private且不遍历junction。Thin06实际退出0、19976ms，7301文件/776目录含根/0links/373049068 B，原cap377487360 B余4438292 B；63205声明记录实际为63187 canonical unique路径，18项同hash斜线别名不能计unique。独立pureFS复核保留44 warning完整多重集及231 JSON实际source/output pairs与Source22一致，仅限定承接旧token结论，未新跑parser/lexer；04/05 route types仍是明确的historical TS输入，不能称零旧路径。

Matrix09当前103 shared/204 legacy/2 open/8 common limitation；其他316整row、317 ordered IDs、全部native/originalAudit历史与rawCounts保持，player全部not-run。有限字段实际312关联/179唯一decl、每row去重311，F01.safe-key原一处duplicate保留；这不是执行数或完整验收分母，无overall percentage、Candidate100=false、goal active。Source22已实际push `4cc32c3f9a3348ebbcd4dce9475acc4c6c7d91ef`；Source23 commit/push在证据创建时pending，由Root后续实际完成并报告。

下一项Source24 Pearl共享准入/钱包与actual Web DOM接线；无经济correlated ACK、Native无限stock同UID/count保持pending是必须明示处理的共同协议限制，不能fake ACK。随后Source25 Ranking Inspect及宽队列。用户暂缓界面操作持续有效：UI/browser/headless/server/renderer、HTTP/socket、WASM API/实例、登录/游戏/保存重登和移动真机及最终frontend验收均未运行。

证据：[矩阵09](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/feature-matrix09.json)、[有限结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/fishing-source23-finite-result01.json)、[动作链复核](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/fishing-source23-action-chain-review01.json)、[组合构建](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/fishing-source23-combined-build-result01.json)。以下Source22及更早段落逐字保留为历史，旧pending、下一步与产物路径只表示当时状态，当前以本段为准。


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

证据：[矩阵08](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/feature-matrix08.md)、[有限结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/bag-belt-source22-finite-result01.json)、[动作链复核](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/bag-belt-source22-action-chain-review01.json)、[组合构建](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/bag-belt-source22-combined-build-result01.json)、[QA索引](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/README.md)。

以下Source21及更旧段落完整保留为历史阶段；其旧pending、下一步和包路径只表示当时状态，当前状态以上述Source22为准。


## Source21：两tooltip源码候选闭合，玩家验收待运行（2026-10-07）

Source21为实际自有Bag/Belt/装备提示接入共享Rust纯formatter、neutral DTO/name表和renderer-free可选PUI ABI1；Native四个公共API与默认ShiftClick选项保留，Web按真实MoreActions入口展示拆分提示。完整document含11类sections/10种colours、原始/真实/解析/socket属性与unknown viewer；raw UID0、signed i32 item index、count/dura/unit sale及完整carrier从实际显示对象独立克隆，catalog/label/slot不能补实例来源。ABI getter custody、canonical i64 .NET ticks与exact formatter clock缓存保持；缺可选能力不阻断普通Core接口，缺字段/partial不伪装完整。

真实Page reader在useLayoutEffect提交后才格式化，调用前后核验物理owner/socket/connection/session/scene/map、Core对象、来源签名、window/tab/epoch和raw对象唯一性；Bag/Belt/Character真实mount传入相应callback，hover/focus/touch只读。active-only1000ms刷新，blur/visibility/pointercancel/resize/pagehide/unmount及旧cleanup不能清掉successor；原basic fallback保留。movement-only itemIdentity纳入sellValue，金币售价单项变化能刷新提示。本批不产生装备/使用/修理/发送权限。

Root选定Rust跨配置169次通过：Core101＋default44（另3项原ignored）＋Native3＋finalPUI21。最终PUI hint修正后只有21新执行，其余148为本次Source21先前实际执行，仅按未变有效配置输入限定承接；不称169都在最终修正后跑过。18项新增distinct、30项tooltip/name执行，原12个Native具名tooltip测试保留（11迁到Core、1留Native）。现有11个Node脚本Node03实际218/218、0失败/0跳过，Stage5原290＋9新增＝299内部组只计一个文件级Node测试，不能218＋299。原clock组名称/断言强化exact formatter clock，旧负index拒绝替换为below-i32拒绝＋合法negative index覆盖；实际组件唯一button AST selector收窄，保留card count1与六events检查，不声称全部旧断言字节不变。Combat23原测试/断言保持。strict code-only非增量TSC03通过。Node01的217/218及失败日志、初始PUI20、旧prepared/build配置完整保留；source-review旧pending句只表示创建时态，最终回执更正当前状态。

当前Core/PUI实际build02通过：Core WASM252417 B/JS24393 B，PUI WASM211995 B/JS16829 B，均满足原WASM<262144 B及JS≤204800 B预算；Core版本cdbc1b7c…、PUI23059247…、source49a8ea7b…，完整hash见组合证据。三renderer实际build02通过原预算，发布版本bevy-e9f57ef01a6282c7。Native EXE实际build01为104760320 B/SHA256`0b54aee401aa2504939dd3dbae80615f597ec83ae19da1321e7170ff43baaf30`；最终rust02仅改PUI，Native有效输入未变，按限定条件承接该实构建，不另称第二次Native构建；EXE未启动。既有.gitignore将Renderer immutable release列为生成资源，未强制入Git；selected Core/PUI四叶提交由Root完成，文档写入不证明发布。

Next04实际退出0（PID101008，142707ms），dist`.next-web-windows-catchup-20261007-04`，strict TypeScript21.9s与13静态页；27404声明输入，19007文件/308目录含根/603079321 B；61 NFT/37094引用（37092文件＋2目录），0missing/private，声明junction未遍历，仅原允许的生成元数据更新。Thin04实际退出0（PID82236，25804ms），包`.mir2-thin-client-web-windows-catchup-20261007-04`，63153输入；7301文件/776目录含根/0links/372903563 B，原377487360 B cap余4583797 B。独立purefs复核63153inputs/19007Nextfiles/7301Thinfiles与11 runtime copies全部一致，0确认blocker、0执行/0写；44 warning完整多重集保持，231JSON实际source/output pairs一致，仅限定承接历史token结论，未新跑parser/lexer。

仅`F05.bag.tooltip-compat`（含装备）与`F05.belt.tooltip`两行open→legacy/sourceCandidate closed。共享formatter不将整个DOM host升级为shared controller；当前103 shared/202 legacy/4 open/8 common limitation。其余315整row、317 ordered IDs、全部originalAudit/native和历史rawCounts保持，player全部not-run；实际row有限字段展平254关联/121唯一声明，每row去重后253关联；F01.safe-key.open原Overlay声明有1处行内重复并保留。旧Matrix06 summary230/118＋新增18/9＝248/127仅为历史summary算术，不是fresh当前总数；旧06实际字段展平236/112。关联数不等于执行、Node218、Stage5299内部组或玩家验收数量。317条只是有界审计记录，没有完整验收分母或总体百分比，Candidate100=false、goal active；源码/CPU有限检查/静态构建不证明可玩。

Source20已实际提交push`463d6f1ce42fefdd4244cbd801f92715e29674a1`；Source21 commit/push在证据创建时仍pending，由Root最终实际报告。下一项Source22真实Bag→Belt，随后`F02.world.fishing-click`、`F09.NPC.PEARL`、`F11.RANKING.INSPECT`，这4项raw open不是总体分母。W6实际UI、HTTP/socket、WASMAPI/实例、登录/战斗/保存重登、移动真机及最终frontend验收仍未运行，用户暂缓界面操作持续有效。

证据：[矩阵07](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/feature-matrix07.md)、[Source21组合结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/tooltip-source21-combined-build-result01.json)、[QA索引](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/README.md)。以下Source20及更旧段落均为历史阶段，旧pending/下一步/包路径不表示Source21当前状态。


## 历史Source20：三维修源码候选闭合，玩家验收待运行（2026-10-07）

Source20已闭合普通维修Bag、特殊维修Bag与维修报价三个有界源码动作：实际Bag DOM button捕获primary mouse/touch、raw UID0/container0/Bag2+40来源，经不透明单用gesture token投放当前75×75目标；维修框为Native176×147布局，Hold/Confirm已接真实控件。最终发送仍受当前owner/stamp/source/geometry/page/scale/DPR/node identity、物理socket UID屏障及精确ItemRepaired ACK门槛保护。terminal先burn再callback，second pointer取消并隔离至terminal和兼容click，close后原Bag button的兼容mousedown保留WeakSet custody。已知金币不足可选择/投放但禁止确认，unknown/locked/busy拒绝；Hold toggle不发送，每fresh accepted drop至多发送一次，正常ACK/dura/gold刷新保留Hold，unknown/owner/mode/close清掉，transient unknown后same key不能复活旧stamp。没有ACK自动重发。

Root实际Node03 11脚本218/218、0失败/0跳过；Stage5原282具名组＋8新增＝290内部组只计一个文件级Node测试，不与218相加。三条旧不可达UI结构断言替换为更强的实际可达Native维修branch AST断言，其余保留；Combat原23测试/断言保留并装配真实Shell依赖。strict code-only非增量TSC03退出0、零日志。Node01/TSC01失败历史保留，prepared02配置未执行。

Root实际Next01退出0（PID77812，112130ms），新dist为`.next-web-windows-catchup-20261007-03`，严格TypeScript13.8s与13静态页；27387原输入fresh hash零漂移，另两项仅允许03 include/route import元数据更新。Next19007文件/308目录含根/602553319 B，61 NFT/37094引用（37092 regular＋2目录），0missing/private；已声明node_modules junction未遍历。Thin01实际退出0（PID103048，28428ms），新包`.mir2-thin-client-web-windows-catchup-20261007-03`，63136输入零漂移；7301文件/776目录含根/0links/372719297 B，原377487360 B cap余4768063 B。独立pure-fs静态审查接受，0确认blocker、0执行/0写，实际复核63136 inputs、19007 Next输出、7301全包与11 runtime leaves；44 warning完整多重集保持，231 JSON source/output逐对字节匹配，仅限定承接历史token结论，未新跑parser/lexer。

Source18 Core/PUI、Native、renderer Rust有效输入图保持不变，421 high-level输入零漂移，本批未新运行其Rust测试或三组构建；默认Core WASM252205 B/JS24393 B、PUI WASM70277 B/JS14472 B按指纹限定承接。Native archive104710144 B只重hash，未启动。当前仅三维修行open→legacy/sourceCandidate closed：103 shared/200 legacy/6 open/8 common limitation；其余314行不变，317 ordered IDs、originalAudit及Native历史完整保留，全部player not-run。这不是完整验收分母，无overall percentage，Candidate100=false、goal active；源码/有限检查/静态构建均不证明可玩。

下一项Source21先提取共享Rust轻量tooltip与renderer-free PUI，接入真实Bag/Belt/Character mount（含装备）；记录集其余6项为`F02.world.fishing-click`、`F05.bag.tooltip-compat`、`F05.belt.move-from-bag`、`F05.belt.tooltip`、`F09.NPC.PEARL`、`F11.RANKING.INSPECT`。W6实际UI/玩家、移动真机与最终frontend验收依用户要求暂缓。Source19 c400与Source18父7567已实际push并经HTTPS ls-remote确认；Source20新提交仍pending，不能称已发布。

证据：[矩阵06](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/feature-matrix06.md)、[Source20组合结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/repair-source20-combined-build-result01.json)、[QA索引](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/README.md)。以下Source19及更早段落是历史阶段记录，当前状态以上述Source20为准。


## 历史Source19：NPC维修路径部分接线（2026-10-07）

现有有限检查218/218和strict TSC通过；Next01、Thin01构建是静态验证，不代表UI操作。Web已接入真实Bag UID、NPC rate/owner、共享Rust quote及精确单用proof/ACK门槛；但固定Windows流程要求Bag drag/drop到普通或特殊维修目标及Hold自动确认，现有Web列表选中/确认不足以关闭行为行。三个F09维修项在矩阵05仍open/partial，修理报价也不能仅因共享Rust quote存在就算动作完成。记录总数与类别不等于完整分母，317行玩家验收均not-run，W6继续暂缓。

## 历史Source18摘要（2026-10-06）

Windows固定基线f72与原Web审计f1cf保留317条有界源码记录：103 shared、197 legacy、9 open、8共同限制；不是完整验收分母，不报整体百分比。Source18补齐大/小地图图像寻路、聊天拖动/4-7-11行/settings draft和Cash预览/转向九条源码候选，规则与控制器共用Rust，Web绘制仍为DOM。选定Rust跨组212次通过、3次ignored仅按有效输入限定承接；11个有限Node脚本218/218、0失败/0跳过及严格非增量TSC通过。Core/PUI两个轻量包、Windows开发EXE、三renderer、Next严格TypeScript＋13静态页及Thin均已实际构建；独立包7,301文件/776目录（含根）/372,680,891 B/0链接，原360 MiB cap余4,806,469 B。源码、输入及静态产物独立复核零确认P0/P1。44条warning内容与重复次数保持，231份JSON仅在source/output逐对字节一致条件下承接历史token结论，未新跑parser/lexer。真实服务/HTTP/UI/WASM实例/账号、玩家流程及移动真机均未验，goal active。见 [行为矩阵04](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/feature-matrix04.md)、[Source18组合结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/presentation-source18-combined-build-result01.json) 与 [QA索引](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/README.md)。

## Progress74：本地 NPC 金币购买 owner 路由已通过有限检查（2026-10-06）

本批 Source03 已通过独立源代码与实际结果审查，0 blocker；现有串行 CargoGuard 下 **3/3 组、53/53 项执行通过**（购买 18、会话 34、商城 1），其中 **16 项新增测试**。两轮历史测试失败保留，不计入 53 项通过数；修正仅涉及新增测试夹具和预期，产品代码保持一致。

网关将本次 Committed / Rejected / Unknown 结果沿真实本地 owner 调用传递；身份、租约和能力检查在执行前完成。提交后的元数据、事件或协调异常保留本次已知结果，捕获前异常保持 Unknown。原 804 行共享通道可逐字节逆向恢复；现有 BuyItem 三字段、四键 JSON、GameShop 及共享尾部处理保持原契约。

本批没有新增真实前端验收：客户端处理回执、Unknown 恢复、当前组合生产构建、实际 Windows/Web 玩家流程以及 Android/iOS 真机门槛继续开放。沿用“继续代码，暂不操作界面”。

本批不计网络购买 ACK、幂等性、客户端恢复、新生产组合、UI/玩家/移动设备验收、Candidate 或总 goal 完成。详见 [本批 QA 及剩余工作](generated/player-qa/client-core-20260930/npc-gold-buy-owner-route/README.md)。

## Progress73：连接内未决购买保护已通过有限检查（2026-10-05）

Windows 与 Web 共用 Core 的 entered/flushed/unknown 保护：同一实际连接上的角色、完整模型、目录、界面关闭和 HMR 变化不能清除未决请求；严格更新的可信连接才可退役旧请求，仍要求新的完整模型和玩家新的明确意图，不自动重发或推断成交/拒绝。旧 holder 契约缺失会 poison 并拒绝；最后来源复核必须是真实完整 checkpoint。

本批 11/11 组完成：Rust 286 次跨配置执行、14 项新增 distinct；Node 256 项、13 项新增 distinct；严格 TSC 与 shared WASM 编译检查退出 0。994 项声明输入中 13 项既有授权变更、981 项保护、0 新源码文件，Source02/实际回执经独立只读复核 0 blocker；Source01 缺陷与原执行证据保留。Node Core 响应是夹具，完整 TSC 消费依赖图未冻结；未运行 WASM Instance。Progress72 的 155 次服务端执行仅限定承接，不算本批新执行。

下一项贯通 Gateway owner lease 的 typed NPC outcome，再接关联/幂等购买回执和完整投影到两端 Core，随后构建新的生产组合。本批不提高整体 parity 百分比；此前“整体 35%”缺少固定验收分母，不再使用。实际客户端、浏览器、移动真机、生产组合及 Candidate 仍未验收，整体 goal 未完成；用户“继续代码，暂不操作界面”的限制持续有效。详见 [Progress73 QA](generated/player-qa/client-core-20260930/npc-gold-buy-connection-barrier/README.md)。


## Progress72 · 2026-10-05 UTC · 普通金币购买 typed 本地处理结果

Session / InProcess Runtime 新入口要求 canonical 认证和 StartGame，默认 unsupported 不执行旧购买；普通 BUY 实际解析 Used 也被 typed 路径拒绝。每笔局部 capture 在真实校验或金币、库存两次提交后产生 Rejected / Committed，incoming_unique_id 仅为入包增量身份。已知结果经后处理 Err 或 unwind panic 仍保留；缺结果为 Unknown，不推断拒绝或自动重发。旧 Buy 内层四字段、binary、特殊经济、GameShop 与原完整收尾保持。

Source02 994 current / 994 after / 992 before，5既有变更＋2新增、987保护；Source01 panic审查问题原样保留且未执行。实际 Simulation153＋Gateway2＝155次测试执行通过，15项新增全执行，独立复核0 blocker；每次 fresh actual C≥50GiB 与原 Guard PolicyB 退出闭环齐全。Web243 / TSC / 共同客户端与WASM仅以未变有效输入限定承接71历史通过，不计新执行。

本批是领域接口与有限检查，未发行公开 processing receipt，未解决客户端 Unknown 恢复、未重建生产组合。下一项同连接未决屏障，再接精确请求关联与两端回执；实际界面按用户“继续代码，暂不操作界面”暂缓。玩家/保存重登/移动真机/最终 frontend / Candidate 未验收，goal active。

详见 [Progress72 QA](generated/player-qa/client-core-20260930/npc-gold-buy-processing-outcome/README.md)。


## Progress71 · 2026-10-05 UTC · 普通金币购买有效期与共同日期精度

服务端普通 Gold Trade fresh 物品按 Crystal 名称标签创建有效期：首个成功括号匹配、Int32 前缀及溢出归零，m/h/d 使用时长，M/y 按日历钳制月末，未知单位使用 Unspecified MaxValue。一次 UTC 捕获用于本笔购买；完整交付载体与 live sidecar 一致，日期超界、距离/金币不足或容量失败均在金币和库存提交前拒绝。带标签模板仅占格，不贡献客户端 fresh-compatible 容量证明。Pearl/BuyBack/Used 保持原路径。

公共 Rust 的 plain UserItemExpireInfo JSON 改为规范 signed i64 十进制字符串，同时读取旧整数；原 Crystal binary i64 不变。Windows/portable 共用同一编解码与严格完整载体门槛。Web 仅在 expiry 单字段形状下用原始 JSON source 恢复旧 unsafe 整数，无 source 时保留 unknown；新字符串原样透传，不放松 UID/count/dura。安全 legacy 数字仍保留既有 JSON 语义，Rental/Sealed 未在本批迁移。

Source01 联合992输入及完整 current/after/before核对：12既有改动＋2新增、978原文件保护。实际 Rust 138＋112＋73＋32＋19＋2＝376次跨配置测试通过，shared WASM 编译通过；17项新增 Rust 具名测试已执行，Native/portable 的5项重复不加成独立场景。Web Node 实际111＋84＋48＝243项通过，parser 的35内部断言组已含于一项文件测试，不另加总；9项新增 Node 与实际严格 TSC 通过。每次 Cargo 使用 fresh actual C≥50GiB、原 immutable Guard PolicyB，首个漏建收据目录的 pre-Cargo 拒绝保留且不计通过。

当前自然 Trade 目录没有 timed 商品，新增事务由 test-only World-local 目录经真实 Session BuyItem 入口验证。本批是源码与有限检查；Progress70 的 EXE/renderer/Next/独立包不能视为包含新代码。下一项先增加服务端直接返回的 typed 普通金币购买处理结果，再接精确请求关联与客户端恢复；完整 NPC 服务与后续组合重建继续排队。用户“继续代码，暂不操作界面”持续有效，实际玩家/保存重登/UI/移动真机/最终 frontend/Candidate 未验收，整体 goal active。

详见 [Progress71 QA](generated/player-qa/client-core-20260930/npc-gold-trade-expiry/README.md)。


## Progress69 · 2026-10-05 UTC · 共同NPC金币买入容量证据

服务端完整快照提供可空的 `npcGoldTradeCapacity` 两字段证据，Windows/Web 原样传入共同 Rust planner。None 保留旧严格身份与仅空格准入，false 禁用；true 只让 listed、身份无歧义且完整 raw Info/载体与 canonical fresh 相容的现代 Bag/Belt 堆叠贡献容量。Bag1/Bag2 同为 container0；Native 保留完整 u64，Web 限安全整数，旧跨 grid alias 仅在 NPC 专用证据门槛内允许。

局部物品/金币 mutation 在任何 handler 前同步撤销可用性，完整同 owner、当前 source stage 与库存指纹匹配才恢复；完整请求与发送 proof 保留证据，只有持久 Core authority 排除它，证据变化不释放 Entered/Unknown 屏障。没有新增购买 ACK、超时重试或 JS 容量算法。

Source02 的 990 当前源码/after 备份及 989 before 完整核对，25 旧文件变更＋1 新文件、964 旧文件保护。实际 Rust 五组 128＋107＋68＋32＋19＝354 次跨配置执行及 shared WASM 编译检查通过；Node 108＋83＋48＝239 项通过，新增 Rust28/Node10。TSC 仅限定承接 Source01 实际通过：Source02 只改四个 mjs 夹具，生产 TS/TSX 与编译有效输入未变，不称新 TSC 执行。Source01 的 19/1/1 失败及原日志保留，夹具精确修复且原断言未弱化。

下一项 Progress70 复用既有有限 builders，按原预算重建 Native、三 renderer、Next 与 Thin；Core24 输入/18 Rust 未改，仅限定承接。生产组合本轮尚未重建；name-tag expiry、无购买 ACK 恢复与完整 NPC 服务边界仍 open。用户“继续代码，暂不操作界面”持续有效，实际 UI/JS sink/HTTP/移动真机/最终 frontend/Candidate 未验收，goal active。详见 [Progress69 QA](generated/player-qa/client-core-20260930/shared-npc-gold-capacity/README.md)。


## Progress68 · 2026-10-05 UTC · 普通Gold Trade整笔购买修复

服务端普通Trade/Gold购买先在库存克隆中完成真实格位、自身数量上限、完整载体及身份检查；兼容的完整现代堆叠依合法Bag/Belt分区吸收数量，剩余量必须有真实空格。全部转换和容量规划成功后才一起写入金币与库存；fresh交付排除catalog UID与0，模板基础属性不写入AddedStats，失败不扣款、不保留部分合并。旧sidecarless根保留各packet grid内的编号，真实exact/nested/reserved冲突继续拒绝，旧物品只占格。Pearl/BuyBack/Used原路径保持。

最终Source04实际购买16/16、相邻NPC115/115通过（115已包含全部16，两个命令131次执行），独立复核0个确认blocker；两次真实失败原日志保留，旧runtime/tests.rs及全局allocator/codec/兼容谓词未改。813 Rust输入与备份匹配，前端553输入未改。

下一项接共同Rust客户端的兼容堆叠容量：由服务端完整快照提供可空roster/compatible-UID证据，Windows/Web原样透传，局部物品包同步撤销可用性，保持Core的Entered/Unknown屏障。客户端满包准入、name-tag expiry创建、无专用购买ACK恢复及生产组合仍需继续；用户“继续代码，暂不操作界面”持续有效，实际玩家/移动真机/最终frontend/Candidate与整体goal保持open/active。详见 [Progress68 QA](generated/player-qa/client-core-20260930/npc-gold-trade-capacity/README.md)。


## Progress67 · 2026-10-05 UTC · 普通NPC商店组合构建

当前共享商店源码的Windows开发EXE、生产Core、三种renderer、Next与独立包完成实际有限构建及独立复核。静态导出测试26/26；Core完整24输入/18 Rust，WASM258772B；三renderer均保留原预算。Next严格TSC及13页面通过；独立包377152588B（359.68 MiB）/7299文件/775目录/零链接，360 MiB预算余334772B。553源码与备份、Core8/Bevy166历史叶保留；完整Next/依赖43897文件核验通过。Next实际只改tsconfig与next-env两项生成元数据，旧失败记录保留。

下一项修服务端普通Gold Trade购买的容量预检与元数据入包不一致，确保整笔成功后才扣款；随后同步共同客户端的兼容堆叠容量规则。用户“继续代码，暂不操作界面”持续有效；当前登录/战斗/保存重登、JS sink运行语义、远端资源覆盖、移动真机、最终frontend/Candidate与整体goal仍未验收。详见 [Progress67 QA](generated/player-qa/client-core-20260930/shared-npc-shop-combined-build/README.md)。


## Progress66 · 2026-10-04 UTC · Web普通商店共同控件接线

Web普通Gold/unlimited/panel0商店已接入Windows同一Rust controller/painter与既有持久Core购买dispatcher；完整584×334布局及两块实际输入区域确认就绪后，才交接整棵React Buy树。arming期间旧树可见但不可输入，键盘、场景定时移动、手柄和移动控件均使用实时阻断；较窄触屏及混合/特殊目录仍整棵legacy。

购买在实际socket.send前核验完整当前owner/service/catalog/source/布局和精确wire，先消费唯一UI proof，最后通过Core entry前checkpoint；状态变化/关闭/换面板/旧pointer terminal不可发送或清除新hold。renderer不创建Core token/slot/ACK，Flushed/Unknown不表示服务端购买成功。Close/Sell延迟退役避免同步回调改写ABI状态，Sell/Repair仍既有入口。

最终选定Node249项通过：Source04新执行仓库接线20+相邻82=102；NPC03 99与Storage03 48在其余986输入及实际消费依赖逐字节未变条件下限定承接，严格TSC03亦限定承接，均无skip。NPC99包含53项新增（buy8/host24/Page-Shell21）。Source02真实NPC/TSC失败与Source03旧夹具16项失败保留，修复未削弱原断言。组合553/Rust812/Web988，联合979；10既有授权/4新增，前批974输入中965保护，9旧变+5新图输入；Rust未改未重跑，仅限定承接Progress65证据。

下一批复用既有有限runner，按固定预算重建Windows EXE、生产Core、三种renderer、Next及独立包。用户“继续代码，暂不操作界面”持续有效；本批无WASM Instance/JS sink真实调用、浏览器/原生窗口/HTTP/玩家/移动真机或新生产组合验收。完整NPC商店、无专用购买ACK恢复、服务端容量/元数据一致性、最终frontend/Candidate与整体goal保持open/active。详见 [Progress66 QA](generated/player-qa/client-core-20260930/web-npc-shop-ui/README.md)。


## Progress65 · 2026-10-04 UTC · portable/runtime普通商店共同host

portable/runtime普通NPC商店已复用Native共同Rust controller/painter，新增独立host与7个ABI出口；原有精确能力对象保持。指针仅提交冻结proof/gesture，Runtime同步回调前后核验完整owner/source、ingress与sink状态，接受后才修改共同选择/数量/分页；renderer只读取既有Core authority/feedback，不创建token/slot/ACK。

最终Source02实际19项portable（含真实字体CPU布局、touch1.28/DPR2、完整584px范围）、17项runtime host、6项能力回归和20项Native共同商店回归通过，合计62次跨配置执行，36项新增；shared WASM编译检查通过。Source01测试所有权编译失败/零执行保留，精确两处as_ref借用修复，未削弱断言。548组合/812Rust输入含同一公共字体，union974；每次fresh actual C≥50GiB Guard PolicyB与child/outer close完成，18个已知自有PID实际查询absent。

下一批接Web唯一共同商店控件树与既有持久Core dispatcher。较窄触屏布局仍整体legacy，混合目录整体交接；Sell/Repair/BUYBACK/USED、无专用购买ACK恢复、服务端容量/元数据一致性保持open。用户“暂不操作界面”持续有效；本批无JS sink实际调用、界面/浏览器/玩家/移动真机或生产组合验收，最终frontend/Candidate及goal保持open/active。详见 [Progress65 QA](generated/player-qa/client-core-20260930/portable-npc-shop-ui/README.md)。


## Progress64 · 2026-10-04 UTC · Native普通商店共同控件接线

Native普通NPC商店现在使用共同controller/painter，保留244×334八行原控件树、raw planner、完整tooltip及disabled无Action；键盘数量与按钮均先退役未entry请求，旧/缺证明控件拒绝。实际UiPlugin computed布局、clipping、font和1.5缩放双尺寸检查通过；没有窗口/renderer或实际点击验收。

最终Source03实际七组88次通过：Native controller/接线20、painter10、购买入口5、NPC对话7、聊天29，portable controller7/painter10。跨配置执行次数不等于独立场景；新controller7、真实Native系统13、真实painter10项。543源码/配置+字体=544，Rust807+字体=808，union970；4既有授权变更/4新增，535/799保护，完整before匹配。每组fresh actual C≥50GiB .NETGuard PolicyB与child/外层close0，27已知PID实际查询absent。

下一批先接portable/runtime普通商店host，再接Web唯一控件树与既有持久Core dispatcher；规范decimal-string Core revision不可转JS Number，renderer不创建新的token/slot。混合目录保留整棵legacy；Sell/Repair/BUYBACK/USED、无专用购买ACK恢复及服务端容量/元数据一致性仍open。用户“暂不操作界面”持续有效，本批无生产组合重建/界面/浏览器/移动真机验收，最终frontend/Candidate及goal保持open/active。详见 [Progress64 QA](generated/player-qa/client-core-20260930/shared-npc-shop-ui/README.md)。


## Progress63 · 2026-10-04 UTC · Web共享购买状态源码接通

Web普通买入已接document/Core版本持久holder和同一Core slot；组件/renderer/HMR重挂载不清已entry屏障，过期來源/旧cleanup/重复点击不能直接发送旧proof。真实Page最后socket前再核验，Flushed/Unknown未当作购买成功。

选定有限检查：Node237（最终Web03新执行38，其余199及严格TSC按未变有效依赖限定承接），Rust51次跨配置通过（最终803图新执行29、Core/Native22限定承接，3项Mail回放ignored）。组合539/Rust803/Web156/Core派生805，联合970唯一输入；12既有变更、525保护、2新Rust，完整before与失败历史保留。

当前QA Core完整WASM258772B/JS19003B，原严格WASM<262144B和JS≤204800B预算不变。固定Binaryen131优化及正式optimizer API均实际通过，产物字节一致；38项构建库测试/语法通过，正式builder已加入发布前/verify预算与优化库指纹。仅QA构建步骤和metadata验证，完整生产Builder/publish、组合产物及实际玩家流程未验收。

商店selection/quantity/page目前仍未共同绘制；只读发现Native数量热键缺未entry撤销，下一批共同controller一并修复。无UI/截图/触屏实际尺寸/真机或JS↔WASM运行时证据，不标frontend/完整商店通过。

下一批先接Native普通商店共同controller/painter，修复数量热键遗漏Bound未entry撤销，再接portable/runtime/Web host。混合特殊货币/有限库存/resale保留整体旧入口；Sell/Repair/BUYBACK/USED、无专用购买ACK结果恢复与服务端容量/元数据插入不一致仍open。用户“暂不操作界面”持续有效，触屏/移动真机、最终frontend/Candidate与goal保持open/active。详见 [Progress63 QA](generated/player-qa/client-core-20260930/web-npc-gold-buy-attempt/README.md)。


## Progress62 · 2026-10-04 · Native商店关闭与购买状态

Native普通Gold/unlimited/panel0的两个购买按钮及真实关闭/换面板入口已连接同一Core attempt。关窗、对话隐藏、Quest历史/选择、聊天Settings和交易接入先撤销未发送请求，再改变可见状态；已Entered/Flushed/Unknown请求仍保留发送屏障。

有限检查通过：Core17、Windows网关28、Native商店64/购买入口5/对话7/聊天29/交易25、portable商店41/购买状态4、Runtime1，以及shared WASM编译检查。合计221次跨配置测试执行，并非221个独立场景；其中Source04新执行176次，Core17与Windows28仅按未变有效依赖图承接，未冒称整图Source04重新执行。

Web新Core façade、共同NPC控件树、报价/传输状态的玩家反馈及实际触屏尺寸仍未完成。React普通买入仍是Progress61路径，当前生产包亦未重建本批改动；无UI/截图/真机证据，不标记完整商店或frontend Candidate通过。

下一批先让Web购买状态接入持久共享Core，再接共同商店controller/painter和portable/runtime host；保留既有价格planner与Native传输边界。用户“暂不操作界面”持续有效，实际玩家流程、触屏和Android/iOS真机、完整NPC商店、最终frontend/Candidate及整体goal保持open/active。详见 [Progress62 QA](generated/player-qa/client-core-20260930/npc-gold-buy-attempt/README.md)。

## Progress61 · 2026-10-04 · 普通 NPC 金币购买共享规则

Native 两个购买入口与 Web 实际页面已接入同一 Rust planner，使用 catalog UID、原始 price/rate、合法容量和完整 raw 来源；Web 在实际 sendRaw 最终一次性 claim，旧目录、owner/模型变化和同步重入不能发送旧凭据。关闭 Native 默认补齐字段/行覆盖 panel、Web 移动快路径漏新对话三处缺陷。

实际 Web 180/180；Rust 各配置 Native64、portable41、Runtime1、Windows投影3、Gateway投影2 均通过，严格 TSC 和 shared WASM check 通过。533 源输入（15既有变更、514保护、4新增），797 声明 Rust 输入经独立复核；原失败与精确夹具修复记录保留。本轮仅源码/有限检查，前轮组合产物不代表新购买接口已构建或玩家验收。

下一项先闭合 Native 确定未入 transport 却遗留 Buy 锁的精确 attempt 生命周期，再做共同 Rust controller/painter/portable host 与无专用 ACK 的观察/未知反馈；另排服务端容量预检和元数据插入不一致的修复。用户“暂不操作界面”持续有效；生产组合、实际玩家流程、移动真机、最终 frontend/Candidate 与整体 goal 保持 open/active。详见 [Progress61 QA](generated/player-qa/client-core-20260930/npc-gold-buy/README.md)。


## Progress60 · 2026-10-04 · 普通仓库组合产物

当前仓库共享源码已构建新的Windows开发profile EXE、三种release+QUEST renderer、Next01和独立包。小Core19输入/完整14个Rust成员/两叶产物未变，限定复用已验收b17版本；新增Storage导出静态校验实际16/16，三种renderer及复制后的三对产物均通过7接口检查。Next Webpack/TSC/13页面通过，独立包376,489,742 B（359.05 MiB）/7,299文件/775目录/零链接，原360 MiB预算余997,618 B。Source529与备份、旧Core6/renderer152保留；新增renderer恰好7叶，现有renderer159。完整Next/依赖43,897文件摘要稳定；独立实际复核0个确认blocker。

下一项代码推进普通NPC金币买入的共同planner/painter/host，先关闭原始price/rate、catalog UID、堆叠容量与无专用ACK的恢复边界。用户“暂不操作界面”持续有效；实际玩家仓库/登录战斗/保存重登、移动真机、媒体覆盖、最终frontend/Candidate与整体goal保持open/active。详见 [Progress60 QA](generated/player-qa/client-core-20260930/shared-storage-combined-build/README.md)。



## Progress70 · 2026-10-05 UTC · 共同NPC容量两端组合构建

本批将 Progress68/69 的普通金币购买与共同容量证据修复构建进 Windows 开发 EXE、三种 Web renderer、Next 与独立包，并完成独立实际产物复核。未改服务端购买规则、协议或 Zone，不据此增加 backend/server parity 百分比。Core 完整24输入及18 Rust成员逐字节未变，仅限定承接既有实际生产构建；不称新的 Core 构建。

Windows EXE 104599040B 已归档且未启动。GPU/GL/shared WASM 为30547434/21966635/31510131B，gzip 为5902438/5384473/6243678B，原 WASM/gzip/JS预算不变。每对当前及复制后的 Storage/NPC 各7静态导出检查通过；Module 检查不代表 Instance 或 JS sink运行。Next实际严格TSC与13页面通过，双编译manifest绑定 Core9191/Bevy1808。

独立包实际377189647B（359.72 MiB）/7299文件/775目录含根/零链接，原360 MiB上限余297713B。完整 Next与依赖43897文件实际流式哈希及最终成员稳定；553源和完整备份匹配，Native814输入、Core8/Bevy173历史保留。Next实际仅改tsconfig与next-env两项生成元数据；无未授权漂移。

下一项修 name-tag expiry创建及 Web JSON日期精度，再处理无专用购买ACK恢复与完整NPC服务边界。用户“继续代码，暂不操作界面”持续有效，实际登录/战斗/保存重登、UI/JS sink/HTTP、移动真机、最终frontend/Candidate未验收；整体goal active。详见 [Progress70 QA](generated/player-qa/client-core-20260930/shared-npc-gold-capacity-combined-build/README.md)。

## Progress59 · 2026-10-04 · 普通仓库源码共享缺口收窄

普通仓库共同 Rust painter 与 Web host/意图出口已接通，React 仓库仅在独立 ABI 不支持、输入未就绪或密码流程等条件下承接兼容操作。桌面/触屏布局、现有密码窗口交接、租赁命令、旧 pointer/owner 回调隔离和最终发送边界通过有限检查：Web 150/150，Rust 113/113、56/56、20/20，TSC/shared WASM check 通过。

仍缺当前组合产物、真实仓库 ACK/密码/租赁体验、字体/像素/拖拽手感、保存重登、移动真机与公会仓库。Web 已经转成 Number 的 expiry ticks 不会由本批恢复原始精度。本批不标记完整仓库、frontend Candidate 或 Accepted。详见 [Progress59 QA](generated/player-qa/client-core-20260930/shared-storage-ui/README.md)。

## Progress58 · 2026-10-04 · 普通仓库存取源码检查

Native 已采用提取的公共 Rust 存取 planner；Web 存取身份、Bag2 格子映射、同步重入、Mail/空目标排他与会话清理已修复，Node 96/96、Rust Native 105/105 与 portable 34/34、TSC 通过。Web 仓库 painter/host 尚未共享，密码/租赁/公会仓库、实际操作与移动证据仍是缺口。本批不标记完整仓库或 frontend Candidate 通过。详见 [Progress58 QA](generated/player-qa/client-core-20260930/storage-transfers/README.md)。

## Progress57 · 2026-10-04 · Shared Mail 组合构建

共享 Mail 的 Windows EXE、生产 Core、三种 renderer、Next03 与独立包的有限构建检查已完成。三种 release + QUEST renderer 使用固定保守优化；修复严格 TSC 误扫描保留独立包的目录排除。实际 Node30/30、既有23处 builder self-check、Webpack/TSC/静态页面及打包通过。新独立包375,942,789 B（358.53 MiB），7,299文件、775目录、零链接，比原360 MiB预算低1,544,571 B；预算未改，原失败记录保留。

本轮未启动界面、HTTP服务或 renderer Instance。当前登录/战斗/保存重登、IME/剪贴板、触屏与移动真机、远端资源覆盖、最终 frontend、Candidate与整体goal仍未验收；按用户要求继续可独立完成的代码工作。详见 [Progress57 QA](generated/player-qa/client-core-20260930/shared-mail-combined-build/README.md)。


## Progress56 · 2026-10-04 · Shared Mail editor/painter

Windows 与 Web 已接入共同 Rust Mail 编辑器和 painter；Web 仅保留浏览器输入适配，完整草稿、金额和发送 proof 仍由同一 Core 管理。修复实际字体负 leading/边框造成的光标越界，并用 Core 当前附带金币值生成快照，钱包余额仅用于输入上限。Source06：470 输入、17 个既有变更、451 个既有保护、2 个新增。

本轮实际 Node 86/86；Rust 共 92 次通过执行（跨配置，非独立测试总数）：portable 29、Native 27、runtime 21、Core 12、三项显式回放 3；另 1 个历史 runtime 环境样例未执行。TSC 与 shared WASM check 通过；84 个 SendSlot 会话的 1,508 对完整 inputJson/outputJson 在 Rust 中逐字节一致。实际 QA Core WASM 259,714 B，严格低于 262,144 B 上限。独立复核 0 个确认 P0/P1/P2。详见 [Progress56 QA](generated/player-qa/client-core-20260930/common-mail-editor/README.md)。

下一项：新 Windows EXE、生产 Core、三种 renderer、Next 与独立包组合构建及固定体积预算检查。用户要求继续代码、暂不操作界面；本次仅验收源码/有限检查，实际界面、IME/剪贴板、触屏/移动真机、最终 frontend、整体 Candidate 与 goal 均保持未验收。

## Progress55 · 2026-10-04 · Web shared Core SendSlot

Web 邮件发送已在 Source04 接入同一 Rust Core SendSlot / fullraw draft clock；旧或已变更草稿的 ACK 只结算传输，清理当前草稿须完整当前 proof 匹配。实际 Node 63/63、Rust 13 次本轮通过（含三项显式回放），另 Core 4 / TSC 仅按未变输入图继承。实际 Core WASM 259,237 B，严格低于 262,144 B 上限；仅移除非执行 name 调试段。Source04 为 468 输入、6 变更、462 保护、无新增源码。详见 [Progress55 QA](generated/player-qa/client-core-20260930/web-mail-send-slot/README.md)。

下一项：合入已独立审阅的 C2 common editor/painter，再验证 Native/portable/runtime/Web 编译及纯代码输入链。生产组合包、实际界面/IME/触屏/移动真机、最终 frontend 与整体 goal 均未据此验收；用户要求继续代码、暂不操作界面。

> 2026-10-01 UTC bounded707 DPR2 browser round is sealed. The quest_ui.rs idle
> mutation guard keeps active turn-in/cancel/stale policy; fixture19/19, turn-in5/5,
> old-source negative control, independent reviews, native/three-WASM/canonical/Next
> and copy gates pass. Root matches365 current/frozen inputs and six archived runtime
> files; the clean copy is376,235,649 bytes /7,300 files /zero links below360MiB.
> Actual600×320 DPR2 sharedGL2 Diary is readable/current across24 distinct samples,
> ordinary close restores Bevy17/17 MP, and its640/844/history controls pass.
> A single Rust4.png abort has305,224ms same-generation endpoints; DOM MP remains
> supported, and separate normal reload restores Bevy MP. Desktop WebGPU secondary
> UI and leanGL2 DOM controls pass. Five accepted cases hash their own JS/WASM
> bodies; case01 body eviction remains rejected. Six ordinary Logout saves
> 21/23/25/27/29/31 equal Wizard2 across11 public fields, excluding HP/MP/experience.
> No movement/item/Quest gameplay or continuous-frame proof is inferred here.
> All seven owned browser-round processes and five ports closed. Raw shutdown bytes
> differ in exactly two derived guildClock fields while accounts/all other decoded
> content match; that raw-byte gate stays failed. No store restore/substituted pass.
> Root accepts corrected fault/recovery v2 (127 inputs/13 references) and final
> matrix (453 inputs); v1 false references/overbroad04 and historical839 failures
> are retained. Case04 has600 controls only; its touch input is the JSONL, close
> JSON is resulting state. Older driver/StartGame/preload causes stay unproven.
> Separate pinned707 native QA case01 receives Wizard5 HP15/15 MP17/17, then its
> QA proxy fails forward_queue_limit.639 metadata rows=636 frame+3 lifecycle;
> the old shared code cannot identify frame-count/queued-byte/single-frame cause.
> No native Welcome/Diary/Bag/normal Logout or retained capture is accepted.
> Disconnect save32=Wizard2 and Warrior260=258 across11 fields. Owned Gateway
> exits0 normally with immediate shutdown bytes equal; this separate boundary
> does not override the browser raw-byte failure. Proxy exited; native window and
> launcher remain unclosed after the user's physical Escape stopped Computer Use.
> v4's4096-frame cap is a bounded hypothesis; v5 scalar queue diagnosis keeps all
> budgets/privacy/FIFO/close policy. Root/separate Sol-high39/39 fake+3syntax and
> root15-file integrity pass. Desktop resumption is pending; tests are not live QA.
> Shared EXP v2 is a bounded source/build candidate:18 product files and369 declared
> runtime/product inputs are frozen; the single writer released source ownership.
> Raw-pair provenance accepts genuine max1 and rejects missing/invalid display defaults.
> Independent source review accepts bounded integration; Web28/28 and Rust15 focused
> fixtures pass (pure tests, not live ECS painting). V1 review failures stay retained.
> Canonical63280e has all three release WASM backends within unchanged budgets.
> Windows debug EXE, Next, and376,348,122-byte/7,300-file/zero-link preboot copy pass;
> actual clean Sharp uses42 files inside the copy. Framework-added tsconfig includes
> are supplemental build inputs, outside the369 declared freeze. HTTP helper v1 was
> rejected before use; v2 cold01 assumed omitted Prguse files were local and is retained.
> Reviewed v3 cold02 passes27 actual loopback GETs, including six runtime bodies,
> startup JS/CSS, font, native image optimization and two same-origin HUD images.
> Those two images use a bounded local fixture via the existing miss proxy, not a
> verified public origin. Both owned HTTP processes/ports are absent. Copy identity
> is preboot; the independent post-HTTP audit matches all7,300 baseline files, with
> one721-byte image cache addition (7,301 files /376,348,843 bytes /zero links).
> No new EXP browser/native/device acceptance is claimed. Commit-triggered refresh
> reduces the polling gap; mixed old-crop/DOM fallback pixels and EXP persistence
> remain actual-client gates. WEIGHT v2 is a separate frozen source/build/HTTP candidate:372 declared files
> and17 product changes against369, with13 read-only references verified at that
> boundary. Four v1 corrections fix mobile interleaving and proven same-player raw
> pair retention through the actual Web projection; both focused Web matrices pass
>36/36. TypeScript/diff pass; Rust is unchanged and prior v1 receipts are reused.
> Windows DEBUG, all three release backends (immutable34b06f, six files83,939,043
> bytes), Next and376,442,716-byte/7,300-file/zero-link preboot copy pass. Clean
> Sharp loads42 files inside the copy. Generated Next type inputs stay outside372.
> Helper01 media assumptions and helper02 H1 cleanup were blocked before use and
> retained. Reviewed helper03 cold01 passes30 declared actual local HTTP checks:
> six runtime bodies, startup assets/font/native optimizer and five source PNGs.
> Prguse/8,1,76 use the bounded local fixture; UI_32bit/473,472 use the verified copy.
> Public media delivery and pixels are not accepted. Actual owned-child close and
> fixture/log cleanup pass; both HTTP-owned PIDs/ports are absent. The independent
> post-HTTP audit matches all7,300 baseline files, plus one721-byte optimizer cache:
>7,301 files /376,443,437 bytes /778 directories /zero links. Whole-copy identity is
> preboot; exact new directory names are limited by the old aggregate-only record.
> The separate Rust draw-plan/Canvas2D v1 remains archived:375 sources and15 changes
> from372, with20 refs. Its D1/D2 BLOCK and cache0/2/gap0/1 failures remain preserved
> beside Web42/42/tsc0. Separate stale-host D3 has two passing fresh controls and one
> failing stale case; no actual device reproduction is claimed. Corrected v2 accepts
> bounded source:375 files, six existing Web changes/369 unchanged,20 refs, independent
> source review and full current/archive checks. Writer and independent final Web49/49
> and tsc/diff0 pass;263 Rust/11 Cargo are unchanged and prior results are reused.
> Root Windows DEBUG and all three canonical releases pass:immutable b3bb1b56, six
> files83,848,758 bytes; prior63280e/34b06f immutable runtimes remain intact. Next and
>376,369,636-byte/7,300-file/776-directory/zero-link package pass the unchanged360MiB
> cap. Generated Next type changes are recorded outside375; lock/buildinfo unchanged.
> Next external asset-manifest trace warning and44 packaging dependency warnings stay
> retained. Clean copied package has all7,300 hashes matched; six resolutions and42
> actual Sharp-loaded files stay inside the copy, with no global/ancestor dependencies.
> Exact776 preboot directory names are archived. Reviewed HTTP passes30 declared
> actual local checks: six runtime bodies, startup/font/PWA/native optimizer and five
> source PNGs. Prguse8/1/76 use three local fixtures; UI473/472 use copied files, so
> public origin/pixels are not accepted. Owned child actual close/fixture/log cleanup
> pass;56208/55872 and19130/19131 are absent. Independent post-HTTP audit accepts all
>7,300 baseline hashes intact:7,301 files /376,370,357 bytes /779 named directories
> /zero links. Exactly one721-byte optimizer cache and three named cache directories
> are added; root parsed all rows and traversed paths. Report fixture-count wording is
> corrected by a sealed addendum. Whole-copy identity is preboot; v1/v2 remain retained.
> The5433/3921-byte supplements stay pinned; no draw-plan browser/native/phone or
> persistence acceptance exists. Current product tip is separately licensed M2 work.
> Mobile joystick M1 v1 froze375 files with two product changes/373 unchanged and
>20 refs intact. Original13/13/tsc0 pass, but independent real-component M1-R1
> diagnostic fails: a retired Run debug timeout overwrites the successor debug view.
> Combined focused command13/14 exit1 remains preserved; no extra game step or
> phone fault is proved. The3718-byte correction licenses only controls/test for v2.
> Corrected v2 source candidate is independently reviewed and mechanically accepted:
>375 files, two existing changes/373 unchanged,20 refs; writer16/16/tsc/diff0 and
> repeated independent diagnostic1/1 pass. Combined raw17 is not17 unique cases.
> Owned Run timer cancellation plus lifecycle/instance fencing retain all original
> movement thresholds and actions. Fresh real start is required after retirement;
> already-issued/accepted authoritative movement is not cancelled.263 Rust/11 Cargo
> and282 native build inputs match draw-plan, so b3bb1b56/six83,848,758-byte runtime
> and the7449 native executable are explicitly reused without a new Rust/native run.
> New Next production build passes with375 frozen before/after source rows. Generated
> two type includes/new dist import stay outside375; package lock/buildinfo unchanged.
> The external asset-manifest trace-copy warning and44 dependency warnings remain.
> New376,372,016-byte/7,300-file/776-directory/zero-link package passes360MiB cap.
> All7,300 file hashes match the fresh isolated preboot copy; six package resolutions
> and42 actual Sharp-loaded files stay inside it. Reviewed new HTTP passes30 declared
> local checks, including six runtime bodies, startup/font/PWA/native optimizer and
> five source PNGs. Three short Prguse8/1/76 fixtures and two copied UI473/472 images
> supply those media checks; public delivery/pixels remain unaccepted. Actual owned
> Next55684/fixture51056 close and fixture/log cleanup pass; both PIDs and19140/19141
> are absent. Root and independent post-HTTP checks accept all7,300 baseline hashes
> intact:7,301 files /376,372,737 bytes /779 named directories /zero links. Only one
>721-byte optimizer cache and three named cache directories are added. The original
> audit used an absent optional check field; its vacuous predicate remains archived.
> An append-only raw-report supplement plus root projection verifies30/30 HTTP200,
>26/26 required hashes and4/4 contracts; corrected audit-v2 is prepared, not run.
> No extra HTTP/tree/OS rerun is claimed. Whole-copy identity remains preboot.
> Prepared HTTP docs were corrected append-only before real execution: product QA is
> first argument, isolated app second. Prior wrong docs/raw receipts stay pinned.
> M2 Web chat IME Enter has a frozen two-file source candidate under the4442-byte
> contract and2664-byte baseline supplement. The inherited375 manifest omitted both
> allowed existing files; that historical boundary and failed read-only pin attempt
> remain archived. Root recovered exact original bytes from its earlier full-read
> buffers, matching independent pre-edit pins; no Git HEAD/substituted baseline or
> source rollback was used. Corrected377 contains the old375 plus two recovered rows:
> exactly2 allowed changes and375 unchanged. Production ChatFrame guards composition
> flag or legacy229 before submission; normal Enter/filter/draft routing remains.
> Independent source review accepts the real ChatFrame-to-GameUiScene send closure.
> Writer focused4/4, TypeScript noEmit0 and two-path diff-check0 are retained.
> Independent review found three original static-test dependencies outside377.
> A new21-reference manifest preserves17 originals and adds four explicitly current
> read-only static dependencies (one overlaps377); these are not pre-edit proof.
> One fresh execution of the same4 tests passes4/4 with actual close0/null and1182
> before/after source/archive/reference/config guards intact; repeated runs do not
> make8 unique tests. Independent addendum closes the current binding gap; full
>377-source/21-reference mechanical audit and root row-level checks pass. Historical
> source-review report keeps its former gap status; the addendum is separate. New
> Next production build and package pass with377 before/after guards and21 references
> intact. Framework type includes/dist import are supplemental inputs outside377;
> lock/buildinfo stay unchanged during Next/package. New376,371,552-byte/7,300-file
> /776-directory/zero-link package meets the unchanged360MiB cap. External asset trace
> and44 dependency warnings remain retained; public media coverage is not accepted.
> All7,300 hashes match the isolated preboot copy, with776 named directories and
> zero links. Six resolutions and42 actual Sharp-loaded files stay inside the copy.
> Seven reviewed label/port-only helpers pass7 syntax and6 fake lifecycle checks,
> separately from actual service checks. New HTTP passes30 declared local checks,
> including26 required body hashes and4 contracts; three Prguse8/1/76 fixtures and
> two copied UI473/472 images supply media. Actual Next55692/fixture46920 close and
> fixture/log cleanup pass; both PIDs and19150/19151 are absent. Independent post-HTTP
> audit and root row/path checks accept all7,300 baseline hashes intact:7,301 files /
>376,372,273 bytes /779 named directories /zero links. Only one721-byte optimizer
> cache and three named directories are added; whole-copy identity remains preboot.
> Audit self-written exit metadata is not used as actual process-close proof.
>263 Rust/11 Cargo/282 native inputs and b3bb runtime
> remain unchanged; six83,848,758-byte artifacts and7449 native EXE are explicitly
> reused without new Rust/native builds or launch. Actual React DOM/browser/device
> IME, native Escape, Android/iOS keyboard and persistence are not accepted.
> M3 portable standalone full-pack trace has a frozen bounded source candidate:
>379 declared sources comprise377 existing plus2 explicitly new paths; exactly one
> config change and2 new helper/test files leave376 existing rows byte-identical.
> All40 current references and archives match. Root and Sol/high independent review
> accept the public awaited post-compile hook for one exact asset-manifest NFT,
> production standalone-only serial tracing and one safe shared descendant glob.
> No private plugin, custom webpack callback, route, dependency or full-pack link
> change was made. Windows fixture proofs are prerequisites, not Next/Linux parity.
> Initial harness esModuleInterop failure1/8 is retained; final8/8 pass repeats the
> same eight cases. Writer direct tsc history omitted _tsc/config input binding;
> root archives those current inputs and one new direct tsc run exits0/null/error0
> with1708 before/after source/archive/reference/supplementary guards intact.
> Current binding is closed without retroactively rewriting prior receipts.
>379-source/runtime aliases are frozen at55b49a97; review confirms zero protected
> drift. Seven exact-derived helpers pass syntax7/fake6; actual services are unrun.
>263 Rust/11 Cargo/282 native inputs and six b3bb artifacts totaling83,848,758 bytes
> are reused with matching hashes;7449 EXE is not launched and no new Rust/native
> build is claimed. One actual normal next build --webpack passes close0/null in
>55,749ms. Its unique awaited public hook filters17 to15 rows before collection;
> final metadata is separately audited across all32 NFT files:61,164 to61,162 rows,
> exactly two full-pack namespace references removed and zero other dependency loss
> or addition. Asset-manifest final30 to28 retains all other bytes/fields/order; two
> large manifests only reorder rows. Independent Sol/high metadata review agrees.
> Runtime version remains unchanged. Next updates only its supplemental tsconfig
> type includes and next-env dist import outside379; source/reference guards hold.
> Actual package exits0 in22,972ms:376,371,942 bytes /7,300 files /776 directories
> /zero reported links, below unchanged360MiB cap by1,115,418 bytes. All904 immutable
> rows match before/after; four supplementary configs are unchanged during packaging.
>44 dependency warnings remain recorded. Actual isolated copy passes all7,300
> source hashes; root independently reads the exact7,300-file/776-directory physical
> tree with376,371,942 bytes/zero links before boot. Six dependency resolutions stay
> inside the app; global and ancestor node_modules are absent. New Sharp child
> close0/null/noerrors executes native import+resize; all42 loaded files stay inside.
> Actual HTTP31 passes26 required body hashes and5 contracts, including one public
> asset-manifest GET. Its9,515-byte raw JSON passes19 named metadata checks and two
> absent-full-node observations; only local unpublished full/remote=false metadata
> is accepted. Original30 requests and three fixture/two package images remain.
> Next46736 closes by owned SIGTERM (exitCode:null, not natural0); fixture55776 and
> both log handles close. Separate actual OS proof confirms46736/55776/53356 absent
> and19160/19161 without listeners. All908 immutable/supplementary plus43 extra
> input guards match after HTTP. Independent post-HTTP physical audit is accepted:
> all7,300 baseline files remain exact;7,301 files/376,372,663 bytes/779 named
> directories/zero links add only one721-byte optimizer cache and its three parent
> directories. Cache SHA matches actual HTTP; preboot whole-copy identity stays
> bounded to preboot. Root parses every baseline/addition/directory row, freshly
> reads the cache/parents, checks996 protected pins and final908 frozen guards.
> Auditor wrapper exit metadata is not credited as independent tool-close proof;
> its unused summary/gate fields are closed by root actual HTTP/Sharp input checks.
> Early auditor script/schema failures and handoff65-hex typo/correct sealed64-hex
> pin remain append-only history. Successful final audit is one audit; no services,
> browser, devices, persistence or full-media tests were rerun or inferred.
> Root first read-only owner proof assertion expected an omitted LocalPort; its
> correction binds the executed fixed-port queries plus actual OS proof and does
> not invent historic fields. Raw receipts/failure note remain unchanged.
> Reviewed HTTPv2 adds exactly one metadata GET to the original30 requests and binds
> the observed route schema; original fake lifecycle6 is reused, not rerun. Initial
> prep schema error, metadata audit cap/path-gate correction, root SHA typo and
> read-only resume binding-name failures remain append-only history. Actual Linux,
> UI/phones/public delivery and persistence remain open; standalone startup is not
> whole-media acceptance.
> M4 document-language source/build candidate is sealed under
> `docs/architecture/web-document-language-host-contract.md` and
> `F:/mir2-cross-platform-20260930/repo-qa-web-document-language-01`. Exactly one
> existing Page gained11 lines/353bytes keyed by its existing locale, plus one
> 13446-byte real-source fixture; all378 other declared sources and51 references
> remain unchanged (380 final sources). Cleanup is one-shot and preserves a later
> different declaration; same-value external ownership remains outside the contract.
> Actual fixture20 unique cases and direct tsc close0 pass; actual tsc1270 listed
> source files exactly match the bound program and1332 archived host inputs. Sol/high
> independent review and root accept the frozen source; repeated logical pin counts
> are not unique files/tests, and ordinary receipt/source association is not a
> fabricated historical per-run self attestation. Earlier harness/lookup failures
> are retained and corrected, not product passes.
> Root actual normal standalone Next16.2.11 webpack completes in58252ms with natural
> exit0/null/no spawn error. All2234 immutable build inputs match before/after.
> The public hook runs before trace collection (17→15, one full directory and one
> descendant removed). All32 final NFT paths retain61162 rows with zero full-pack
> namespace rows; ordered-multiset dependencies/other properties equal the bound
> M3 metadata, with only two row-order differences. No trace-row target is followed.
> Generated SSR8282bytes keeps html lang=en/translate=no/notranslate; this is built
> HTML, not hydrated browser DOM. Next adds only its two type includes and changes
> the next-env dist import; direct tsc buildinfo340394bytes remains unchanged during
> Next. Existing b3bb runtime aliases/archives are reused, not rebuilt or launched.
> No package/copy/Sharp/HTTP rerun is needed for this bounded CSR metadata change;
> M3 isolated31 requests remain historical M3 evidence, not new M4 client acceptance.
> Four-language Login/Select DOM, saved preference/reload, route cleanup/hydration,
> assistive reading, native locale/HUD/Login/save, touch/renderers and physical
> Android/iOS remain open; desktop resumption is pending and goal stays active.
> Root also accepts bounded locale inventories:15 inputs/36 exact excerpts and
> four-language key/value coverage; native handoff10 new inputs plus3 prior references
> (one omitted prior Bevy DTO reference explicitly added by root),26 exact excerpts.
> Shared Quest chrome/Chinese ID remapping and separate navigator-based PWA copy are
> proven source boundaries. Simulation supports four LanguageCode values but the
> client host had no corresponding locale carrier before M5; inferred native default
> was not a runtime observation. M5 shared Quest presentation-locale SOURCE was
> licensed under docs/architecture/shared-quest-presentation-locale-contract.md
> (10987B / 476612a7). Root verified proposal01:21 current/archive inputs,28 excerpts,
> six output pins; proposal02:nine reused inputs,38 excerpts,36 exact function
> signatures,22 canonical four-language keys and six output pins. No proposal ran
> tests/builds/clients. Baseline392 entries =386 full current archives +six new
> absence entries;57 protected references. Sol/high held the initial12-path source lease;
> independent Sol/high reviewed the contract/source, Luna/medium archived
> a source-backed QA matrix. Root owns integration, docs and artifact promotion.
> The shared LanguageCode presentation Resource defaults legacy native/missing DTO
> to zh-CN; new Web supplies exact en/zh-CN/es/pt-BR, rejects alias/null/invalid/
> duplicate fields and probes capability v1 before serializing to an older runtime.
> Locale-only revisions must redraw and invalidate old compact/input stamps without
> resetting open_revision/scroll/selection/reward/pending. Explicit context/raw leaves
> prevent second Chinese remapping of selected Quest chrome/non-Chinese display and
> bypass Chinese text/name for opaque NPC/player/custom strings. The exact22-key
> generated subset comes from unchanged canonical mirrors; no full-bundle WASM lookup
> or dictionary/native-preference/backend/session/auth/save modification is licensed.
> Unkeyed tabs/back/primary/confirmation/journey/practice/supply/tooltip and PWA/HUD/
> full content remain OPEN. M4 Page effect must stay byte-identical. M5 source tests,
> independent frozen code review, native DEBUG/all three release WASMs, bound tsc,
> Next/NFT/clean standalone and actual client/device/save gates are not yet accepted.
> Existing31MiB per-WASM and360MiB standalone caps remain; M3 clean376371942B
> leaves1115418B headroom, not a prediction of final M5 build/package delta.
> Sol/high owns bounded changes; Luna/medium audits evidence; root integrates and
> verifies source/build boundaries. Actual device/UI/persistence and full media stay open.
> Ordinary native Login, trusted capture/human inputs, phones, partial/zeroMP,
> MP/EXP/WEIGHT persistence, full locale/HUD, compositor/performance/public delivery
> and CP02–04 remain open. Desktop resumption is pending; goal active.
> M5 review/recovery checkpoint (2026-10-02): F volume has0 free bytes; actual
> Cargo101 logs retain no-space and rustc stack-buffer-overrun diagnostics. No
> files/cache/evidence were cleaned, moved or deleted. A fresh C continuation
> has measured disk margin; sequential Cargo uses its new target with a50GiB
> pre-command floor. C runtime compiles and actually executes16 tests:15pass/1fail
> because the new inGame DTO fixture omits the required player, not a production
> validation bypass. Its raw failure is preserved; final checks must be rebound.
> Root stable392 full-byte archives show exactly12 authorized changes,380 other
> sources/57 current+archive references unchanged, and the M4 effect byte-identical.
> Independent Sol/high initially blocked source acceptance on four actual-source findings:
> four scroll count sites still use legacy Chinese; three desktop close/Quest
> captions bypass current chrome; manual App fixtures lack the locale Resource;
> two Quest body paths lost legacy zh conversion. Valid scroll positions must
> survive locale-only changes; bounds use the currently rendered body. Root grants
> only four existing paths for these fixes, new real regressions and the minimal
> player fixture;388 stable paths remain protected. Unique writer stays Sol/high.
> Review evidence1290 before/after rows includes1284 explicit external pins and six
> observed-only rows; Root checks1316 unique byte files,14 exact excerpt groups and
> the two Page hunks. A sealed append corrects Page prose: actual object shorthand
> language and callback dependency language; snapshot.language belongs to bridge
> code. Luna/medium reviews the C helper five-site derivation/unchanged copied Next
> and package helpers. Syntax-only exit0 is not a client build/child-close proof.
> M5 source/independent repair review/native/WASM/Next/NFT/package/client/save/device
> acceptance remain false. Existing31MiB/360MiB caps stand. New QA is
> C:/mir2-cross-platform-storage-20261002/repo-qa-shared-quest-locale-boundary-02;
> original F artifacts remain historical and unchanged; GUI resumption is pending.
> Goal active; no frontend Accepted, no full parity percent and no deployment.
> M5 compatibility checkpoint: Root independently verifies sealed repair review02,
> 2867 logical inputs,1581 own full archives,1593 pinned outputs and4454 unique
> full-byte reads. Four initial repairs are resolved in observation02: only two
> repair files changed and390 other source entries/57 references remained stable.
> Quest body compatibility includes three sites over completion/unknown fields;
> repaired04 is historical489391B, while final portable21/tree4/integration4
> actually bind490610B. Node19/generator22/M4-controlled20/actualCLI tsc close0
> are evidence for their used inputs; they are not native/browser acceptance.
> Final native adjacent actually closes101:79 executed,78pass/1fail at the
> unchanged legacy section test. Root and independent Sol/high establish that
> both Chinese Time Limit paths lost Blank/Heading/Body structure; distinct
> Progress was conflated with Objectives. Progress is a static second defect,
> not a second executed failure. Old assertions must remain; do not weaken them.
> Root grants only quest_ui.rs for two Chinese section branches plus the distinct
> existing Progress heading and meaningful current-builder regressions. Other391
> source entries are protected. Non-Chinese canonical time formatting stays;
> unkeyed Progress remains authored legacy Chinese and future locale work OPEN.
> Sol/high remains the sole source/Cargo writer in fresh implementation04. All
> final native/portable/runtime runs and392 frozen aliases must bind the final
> corrected source before independent acceptance or any new client build.
> Read-only PWA locale proposal is sealed and Root-verified:8 inputs,14 exact
> excerpts and14 outputs. Architecture remains undecided and code unlicensed;
> installing/offline/device behavior was not tested and worker caches no HTML
> navigation shell. Current route/install-listener timing must be settled before
> moving the PWA shell mount. Existing31MiB/360MiB budgets remain unchanged.
> Source/build/client/save/mobile acceptance remain false; desktop resumption is
> pending. No native/browser launch is authorized. Goal stays ACTIVE.
> M5 final SOURCE checkpoint (2026-10-02): independent Sol/high cumulative review05
> recommends the exact freeze. Root verifies11497 logical observations:
> 11479 explicit expected pins and18 observed-only stable rows;9422 normalized
> path inputs (not inode dedup),200 full own archives and208 output pins. All
> final392 current/worker/Root archives match; original386 archives bind exactly
> twelve changed paths including six new files,380 protected sources unchanged.
> Current57 references/archives remain stable. Source aliases are byte-identical
> 255669B /64f32eb1d7bcef89ffbfe05ba1a0fd09b2cc26d967247a4a8c210f04d78e8fba,
> SOURCE manifests rather than WASM manifests. Root accepts source and independent
> source review only; worker historicalfalse metadata remains sealed.
> Final11 commands actually close0: Rust host16/portable integration4/mobile21/
> tree6/native integration4/adjacent79/tree6; Node19/M4controlled20/generator22/
> actual CLI tsc1360rows=3before+1357reads. Root verifies132 per-run12 source
> current/archive bindings,2023 current outputs and6106 retained historical outputs.
> These repeated feature runs are not summed as unique coverage. All old failed
> runs,zero-test nonpass,withdrawn expectation-change proposal and corrections stay.
> Actual14 outer tool closes have4 fulfilled/value and10 direct shapes:13exit0,
> one preserved negative outer1/childCargo101, no running worker sessions. Page
> report prose is corrected by sealed Root append: object shorthand language and
> language dependency; the M4 effect is byte-identical at its350/352 boundaries.
> Writer releases source/Cargo ownership. Root licenses sequential local native
> DEBUG/three canonical release WASMs/production Next+NFT/fresh standalone only;
> native build started with measured C253717393408B free,50GiB floor. No new
> native/WASM/Next/package build is accepted yet.31MiB/WASM and360MiB package stand.
> PWA refined proposal02 is Root-verified evidence only:8 frozen reused inputs,
> nine exact excerpts,four outputs. It corrects global-mount interpretation:
> existing install/display listeners are pathname-gated to / and cleaned on route
> change. Candidate lifecycle-only Root context preserves gate/timing/state;
> visiblePage shell would receive its single existing language. No language mirror,
> all-route capture,new dictionary/offline HTML behavior or code license is granted.
> Actual browser/native/device/pixel/install/persistence checks remain OPEN; GUI
> resumption pending. Overall/frontend Candidate and Accepted remain false; goalACTIVE.
> Root preparation update 2026-10-02 04:35 UTC: native DEBUG remains running
> (owned exec session11505); no new native/WASM/Next/package result is accepted.
> Actual C free-space observation251192930304B (tool4e6567) satisfies the fixed
> 53687091200B floor. No cleanup, cap increase, GUI/client launch or deployment.
> PWA proposal03 draft evidence accepted only: eight frozen full archives, nine
> exact excerpt groups, five output pins; contract6944B/a4521efb259ebf79fb0971f66d4f0d16a76b8131c02d0d313ca087d6bffeab7a.
> Draft retains root / route gate and pending install lifecycle; Page language is
> single owner, semantic status uses latest copy, es English fallback remains OPEN.
> No M6 product implementation license until M5 build gates.
> Sol medium completed QA-only standalone and NFT preparation; Root verified25
> complete standalone inputs/33 outputs/seven exact HTTP replacements, and41
> NFT inputs/48 outputs/32 historical M4 raw archives (61162rows/4201100B).
> Root actual Node --check passes (1b85e3,2d1f67,71c2f7) and PowerShell static
> parser pass4f5046 are syntax-only. No helper execution/service/NFT/HTTP pass.
> NFT draft records canonical and other subtree copies separately, full raw/order/
> multiset/nonfiles deltas, no target operations, no assumed M5 count. Observed
> configured tracingRoot is outer repository; verify final generated config later.
> QA helper source/evidence review and syntax receipts live in C continuation
> standalone-verification-preparation01 and nft-verification-preparation01.
> GUI resumption remains pending; actual play/device/save/Candidate/Accepted remain
> OPEN. Next bounded implementation/review uses Sol high after Root build gates;
> total goal remains ACTIVE.
> Root native build accepted 2026-10-02 04:49 UTC: owned exec11505 actually
> completed exit0 (b7ce43); Cargo child58344 spawn/exit0/close0, no signal/error,
> pinned1.95.0 locked/offline dev optimized+debuginfo elapsed2534983ms.
> Root independently hashed both target and QA executable:103561216B,
> ec0981d0d656ebe6239de4e312f6217569a2286cd7433a5d67c6c70a8a5a7d92.
> All392 Source rows and57 reference rows/current+full archives match before/after;
> 449logical rows cover443unique paths (six Source/reference overlaps, none dropped).
> The Root pathname-key assumption correction is retained in native-build01; it
> was a read-only validation correction, not a product/build change. All19 old
> runtime pins and Source-only alias bytes are preserved. Compiler warnings retained.
> Root receipt root-native-build-acceptance01.json157296B/
> 1736bd2189dd01226bc0c16cf581cbbfa8e9f29943f14cb9ca5716316abb405e.
> Native build acceptance is Source-bound compilation only: executableLaunched=false,
> actual client/save/device/Human acceptance remains false/open.
> Canonical three-WASM build started04:49:17Z (4e0a6e, owned exec75733), actual
> C free246606901248B exceeds fixed50GiB floor; one Cargo writer/jobs1/inc0/64MiB
> stack. WebGPU stage observed first, release canonical helper remains8278B/
> 2eb8eb277099d205894f33dc9b6d668beb5f388a7ca302cdedef33f7f309a0fd.
> Native gate is accepted; canonical/31MiB-WASM/Next-NFT/fresh360MiB package gates
> remain pending, aggregateBuildAccepted=false. M6 license remains pending those
> gates. GUI resumption pending; overall Candidate/Accepted false and goalACTIVE.
> Actual canonical failure retained 2026-10-02 05:04 UTC: first WebGPU stage
> Node child44540 exit/close1, no signal/error; outerexec75733 actuallyclosed1
> (4b0c94), elapsed511294ms. stderr28472B/
> 1e085ac0b20f65b3a0aaf52fbe64b230f296685a030bcbe2c90b25c3cdd7ae73.
> Compiler E0599 quest_ui_host.rs559: cfg(wasm32) snapshot ingest now has17
> top-level system parameters. Root verified actual Bevy0.19.0 function_system.rs
> line950 supports0..16, and system_param.rs supports resource tuples. Native
> build/tests did not compile this target-specific registration path.
> Before repair Root rehashed all392Source+57reference rows and all19 old runtime
> pins unchanged. No newWASM/publish/Next/package acceptance; lock absent normally.
> Failure stdout/stderr/result and Root full post-failure guard evidence stay
> sealed in02/root-canonical-failure-evidence01.json; no expectation/cap weakening.
> Root opened fresh C repo-qa-shared-quest-locale-boundary-03 with all392 full
> before backups,57reference manifest and complete local framework evidence.
> Sol high single worker may group applied+locale into one tuple SystemParam only
> in wasm32 function signature; entire function body/other391Sources/57refs/old
> assertions remain protected. Actual wasm32 check matching webgpu,web-quest-ui
> canonical features and original16native-host semantics required; oneCargo/jobs1/
> inc0/64MiB stack/50GiB fixedfloor/lockedoffline. Root owns rebuilt Native/WASM/
> Next/package, and Sol high independent read-only review follows final freeze.
> Parent64fSource/native acceptance stays historical and source-bound; current
> repair Source/review/build is not yetaccepted. Unchanged JS/TSC/otherRust tests
> remain identified as historical, never relabeled byte-bound to repaired host.
> M6 remains unlicensed until repaired M5 build gates; actual GUI/client/device/
> save/frontend Candidate/Accepted remains OPEN and total goalACTIVE.
> M5 repaired SOURCE checkpoint (2026-10-02): QA03 worker and independent Sol/high
> final review are sealed. Root verifies392 current/final/before source rows plus
> 57 current/archive references,433 worker outputs,49 reviewer direct inputs/own
> archives and56 reviewer outputs;1821 before/after reported observations match
> actual Root readback. The only repair is wasm ingest applied+locale tuple17→16;
> complete4653B body and391 other sources stay byte-identical. SOURCE-only aliases
> 331882B/39f56e81dc7a2f408ed1c1db0383dadb77e6f86cc2ce8d822224aa7575e4fd6c
> accepted, including independent source review. Actual specified WebGPU check
> closes0; existing native host16/16 pass. Inherited command backup/repairChanged
> metadata is phase-before, not final archive; actual hashes bind source-after.
> Worker full Rust env purge was not recorded; Root final builders sanitize Rust
> override names. Old11 test commands remain prior-source history, not new runs.
> [Repair evidence](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-quest-locale-boundary-03/root-source-acceptance01.json).
> Fresh repaired-source Windows Cargo build actual child/outer close0,449 domain
> guards preserve392 sources+57 references across443 distinct filenames. Warm
> native result remains103561216B/ec0981d0d656ebe6239de4e312f6217569a2286cd7433a5d67c6c70a8a5a7d92,
> consistent with wasm-only signature repair; target and freshQA copy each hashed.
> Neither was launched. Root accepts native build only. Three canonical Web
> packages are running in actual exec61177; precommand C free245405192192B exceeds
> fixed50GiB floor. Source-only acceptance is distinct from pending canonical/Next/
> NFT/fresh360MiB package/copy/Sharp/HTTP gates. Old runtime19 files remain old
> b3bb before builder. Fixed31MiB perWASM/360MiB package caps remain unchanged.
> Root owns the sole Cargo/build lease, product writer released. Sol/medium is
> doing a read-only Android/iOS boundary inventory. M6 remains unlicensed until
> repaired M5 build gates; actual GUI/player/device/save acceptance and Candidate/
> overall goal remain OPEN. Escape desktop consent remains unanswered.
> Root repaired M5 build update (2026-10-02): actual canonical exec61177 closes0
> (1b215d), Node58692 spawn/exit0/close0; independent Sol/high final review and
> Root readback960 observations/24 outputs accepted canonical-only. Runtime is
> bevy-4e3a46d90b6b8134, schema2 metadata1711B/dee14eba4231489e3781e0523a6b40a7f70cc38c65b04adf81738386340b0f76.
> Source-only39f56e81dc7a2f408ed1c1db0383dadb77e6f86cc2ce8d822224aa7575e4fd6c
> remains bound to392 rows. Final WebGPU30541483B/WebGL221442182B/shared31561948B
> each satisfy unchanged32505856B cap; shared headroom943908B. Old immutable/F
> runtime13 protected files remain old b3bb; actual build lock absent normally.
> Next actual27741/d4f6ea closes0 in64502ms,13 static pages; Source392 and new
> runtime bytes unchanged. Only generated tsconfig includes and next-env routes
> import change, reviewed by Root; config/TS buildinfo unchanged. TraceRoot is
> actual outer E:/mir2-player-journey, NFT rows resolve from each NFT directory.
> Whole-dist NFT actual acac3a exit1 is retained: source standalone node_modules
> junction prevents complete full-tree metadata enumeration. Canonical32/61162
> rows compare raw/order/multiset/properties exactly to M4;29 copied metadata
> entries compare to canonical, but do not close the junction. Root-reviewed
> strict canonical-only helper actual dba43d exit0 covers root NFTs+server; all
> scope links/nonregularNFT/errors block. Its coverage is canonical only and
> widerWholeDistAudit binds original85374B/9fc71784ecf5a0ae45006a2c7528e480e9894b4541a62c1c9343ec25986c387a exit1.
> No target operations or whole-dist success is inferred.
> Actual package23003/f21739 closes0:376400980B/7300files/776dirs/0links below
> unchanged377487360B cap, headroom1086380B; trace warnings44 retained. Fresh
> outside-checkout physical copy98543/e76f18 closes0, all7300 hashes identical,
> no ancestor/global modules;6 dependency resolutions internal. Actual Sharp
> b694b0 child57788 closes0,42 loaded files verified internal; native resize2134B
> succeeds using installed0.34.5 (declared0.35.3 mismatch remains recorded).
> Actual cold HTTP f267ee closes0,31 selectedGET pass against immutable runtime,
> startup/font and static bodies/contracts. TwoPNG are packaged; threePrgusePNG
> are exact localfixtures through same-origin miss, not public origin coverage.
> Owned server59596 closesSIGTERM; fixture56368 closes, logs close. Independent
> OS15427b confirms ownedPIDs59596/56368/57788 absent and19160/19161 no listeners.
> Full physical postHTTP audit70f82b includes every cache:7301files/779dirs/
> 376401701B/0links, original7300 unchanged, one721B image cache+3dirs added;
> whole-tree-identical=false, no cache exclusion. Root frozen pipeline69kB/f63f16e6ea4f1d27a1b1e3045611cd3fed245dd3ce7dbd73b3345dc12347e9c6
> awaits final independent standalone review; aggregateBuildAccepted remainsfalse.
> Root read-only receipt/filename/reconstruction mistakes and actual first
> copy224b89 MODULE_NOT_FOUND remain in diagnostic/recovery receipts; actual
> corrected file invocation closes0. No product fail, old result or scope was
> silently rewritten. [Build evidence](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-quest-locale-boundary-03/root-standalone-pipeline-frozen01.json).
> Sol/medium Android/iOS inventory accepted as evidence only: Capacitor uses
> remoteWeb; Root primary route preference is sharedWindows/WebRust plus thin
> Capacitor device adaptation. Separate AndroidBevy host has unfinished production
> lifecycle/input/effects/transport connections; no nativeiOS host observed.
> Device execution and implementation remain OPEN. M6 verification feasibility
> preparation accepted: actualTSX controlled hooks/context harness is possible
> with existingTS5.9.3/React19.2.3;12 cases planned, no component cases executed.
> Preserve uncanceled350ms orientation timer and deniedStorage later-timer behavior;
> current3copygroups/esEnglishfallback stay explicit. M6 still unlicensed until
> final M5 build gate; next single writer Sol/high, independent review Sol/high.
> Root applies React best-practices checklist for lifecycle/state ownership.
> All GUI/native/browser/device/gameplay/save/publicdelivery/Candidate/Accepted
> remain OPEN; Escape desktop consent unanswered. Overall goal remainsACTIVE.
> Root final M5 local-build checkpoint (2026-10-02): independent Sol/high
> standalone review7241B/48c60056e2171e75fa5a5cf84b9b7a143b404b64767e142298fcc68a308db464 recommends aggregate local build.
> Root actual1353input and161output observations all match; Root accepts complete
> source-bound localNative/canonicalWASM/Next/canonicalNFT/physicaldistribution/
> selectedSharp+HTTP pipeline in root-aggregate-local-build-acceptance01.json
> 408067B/ed1ee18f34019288ce1f0652f8542c643c3b1d6b653deb4c67dd8606e832671b.
> Existing whole-dist acac3a exit1 remains incomplete; current strictcanonical
> scope and materializedphysicalpackage closure are separately accepted. Reviewer
> selected70unique physicalfiles supplement Root real7300copy comparison; no
> fabricated reviewer7300physicalreads, whole-tree-identical or gameLogout/save.
> Reviewer naiveURL inventory error/correctedactualproxy/static mapping stay sealed.
> Actual GUI/player/device/install/save/publicdelivery/Candidate/Accepted remain OPEN.
> [M5 local-build acceptance](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-quest-locale-boundary-03/root-aggregate-local-build-acceptance01.json).
> Root licenses M6 in fresh C repo-qa-pwa-selected-locale-01, effectivecontract
> 11653B/7f438c42f473e1a80b5f25ceb5b115af34771cb4e7ac06f964e81395cadd3088 and grant2832B/2a6218db484e7ba165721bfb23dac21d61271c2243bead5f0ce7ac8149a8cefd.
> Exactlyfive paths: PWA module/layout/Page/oldPWA test/newPWA locale test. Root
> Capture keeps existing / routegate,7listeners/5000hint/uncanceled350orientation
> and deniedStorage later-timer behavior; visiblePage consumer follows its sole
> canonical language and derives semanticstatus copy eachrender. No locale mirror,
> screen/locale key, newdictionary/dependency or CSS/worker/metadata change. Es
> Englishfallback remains OPEN. Pendinginstall/fullscreen state survives locale
> and consumer changes. Page onlyoneimport+onemount before OriginalClientShell.
> Root393before rows=392current fullyverified reusedM5fullarchives+absentnewtest;
> 388existingprotected. New57reference fullCbeforearchives rebase only licensed
> M5 currentmetadata to4e; oldreference/M5/M4 archives unchanged. All27new/old
> current/immutable/F runtime pins protected; M6 reuses accepted runtime exactly,
> noRust/native/WASM edit/rebuild or newpass inference. HistoricalpreM4 Page negative
> control655847B/bd9db38ccb4c72ebb18fd6157785e692d76f0c582f438d726f8a5651dbc4f8ab copied into newQA test-reference for actualdoclanguageCLI.
> Sol/high singleSource writer nowimplements exactfivepaths plus actualnew/oldPWA,
> boundedtsc(no incremental output),questlocale anddoclanguage tests. Sol/medium
> prepares newNext/package/NFT/copy/Sharp/HTTP helpers inQA only; secondSol/high
> prepares independent source review from frozenbefore, no live5unstable reads.
> Root architecture/integration owns leases/globaldocs and later freshWebbuild.
> GenericcontrolledTSX harness must execute actual callbacks/context/effects,
> queuedObject.is hooks and deferredPromises; realReactDOM/hydration/device remains
> separate. M6 Source/test/build are not yetaccepted. NoGUI or services licensed
> toworkers; desktopEscape consent remains pending and overallgoal staysACTIVE.
> M6 pre-implementation protection correction (2026-10-02): independent
> Sol/high preparation finds actual393before inventory omitted Layout, which was
> only a57reference entry. Original388protected count and statement that all57
> currentreferences stayfixed were inconsistent with alreadylicensed Layout change.
> Root immediately paused newworker edits/tests; worker confirms only QA/preflight
> a8fbad close0 had run, no productfile changes/newtest/tests/ongoing process.
> Root preserves original393before/grant/contract/preflight immutable history,
> adds Layout exactfull pregrant1434B/70734ba87af5abfbb7943dac2a0ef54d827fd363ad967f4c1b1867b588831c23 from C reference-before,
> and seals authoritative source-before-v2.json394/b62158e6e4c39ee67dd40631f07655dc0c68689a3520f42d3afb2570355e61cf.
> FinalSource mustinclude394 rows;389protected existing Source stayfixed. Only
> currentLayout is an authorizedreference change and mustequal finalSourceLayout/
> fullafterarchive/precise reviewedlayoutdiff. Other56current refs includingM5
> runtime4e and all57before archives stayfixed;27runtime guards remainfixed.
> Source/reference obligations use domainkind+path despite duplicates; never
> silentlywaive allrefs or reinterpret393preflight as394 coverage. Five authorized
> productpaths unchanged. [Supplement](C:/mir2-cross-platform-storage-20261002/repo-qa-pwa-selected-locale-01/root-grant-supplement02.json)
> 2892B/13ae72e219667459dad8388c4b259a4c5edd4a846afa10d3b04d3cbf858e3867 and contract-supplement02.md2745B/aa0a5a2fd7f36a63cb0a16dfd3c598dcccba0f03b065ee5beeaacd462ab7e43c
> supersede originalcounts/rule; Root didnot read/write live5files to repairmetadata.
> SingleSol/high worker nowresumes originalimplementation and fresh394preflight;
> readonlyreview andQAhelperpreparation bind supplement. M6 Source/build/client
> acceptance stillfalse, parentM5localbuild acceptance unchanged, overallgoalACTIVE.

> M6 preparation independently reviewed by Root:42+13 helper-preparation inputs,
> their complete archives,60+20 outputs, and25 source-review inputs+25 ownarchives/
> 33 outputs all match actual full-byte hashes (273 recorded observations). Six
> helper derivations replay exactly; two reused helpers are byte-identical. All10
> helper sources were reviewed. Eight Node syntax checks and two PowerShell parser
> checks actually exited0 (outer d9a501/05d578); no helper build/network executed.
> [Preparation acceptance](C:/mir2-cross-platform-storage-20261002/repo-qa-pwa-selected-locale-01/root-build-and-review-preparation-acceptance01.json)
> is107324B/2dacd1b2e810bc4c4944f4c4cbc5fb9a62496ccb607c5e528031ebe3948ba7ea.
> Root QA reader initially omitted the separate completeOwnArchives domain; its
> diagnostic and corrected83 checks are retained, not a product failure. The next
> pipeline requires Root-accepted actual394 frozen sources, exact path-set/layout
> dual-domain equality, unchanged runtime4e/27 guards, fixed31MiB WASM and360MiB
> package caps. Canonical NFT comparisons use actual M5 parent32 metadata; a changed
> M6 count/content must be reviewed rather than forcing32 or claiming whole-dist.
> Worker-reported fresh394preflight and corrected selected-locale counterexample
> remain pending Root receipt review. A generic React19 Context-metadata harness
> failure is retained and being repaired only in the licensed new test; no product
> expectation weakened. Sol/high owns sole five-path implementation and later
> independent Source review; Sol/medium now prepares a bounded read-only next
> shared Quest-copy inventory from frozen M5 archives. M6 Source/build acceptance
> stillfalse. Real browser/native/PWA/device/save/Candidate remainOPEN; goalACTIVE.

> M6 Root Source-only and independent Source review are now ACCEPTED. SOURCE-only
> aliases497276B/d7377e17b64bdd35a6bc492ff18e67d87f8e926fc7f9b5e9427bc8a3b09f6686 contain
> current394fullafter/393fullbefore; exactfivechanges/389protected. Layout is
> independently verified in Source/currentReference while57before archives remain
> fixed and other56current refs+27runtime remainfixed. Root actual1322logical
> guards,1580workeroutputs,4414independent physicalafter inputs and521outputs match.
> Page importblock includes its separatornewline; removing it and actual4space
> mount restores656228before bytes. Layout twoedits andoldtest oneassert restore
> entirebefore files. PWA root listener/device/state helpers and24authored strings
> match; semantic status and Page-derived visiblecopy retain both install awaits,
> both fullscreen/landscape awaits,absence/route/storage/original timer behavior.
> [Source acceptance](C:/mir2-cross-platform-storage-20261002/repo-qa-pwa-selected-locale-01/root-source-acceptance01.json)
> 4765B/823b9a927e718f8d4b4bd4517e14cb2d39c6cd663d279a62b60e4852e571bb6d binds
> final fiveactualexit/close0: PWA16/16 (2generic14product), Quest4/4, doc20unique
> cases withactualpreM4negative, actualTSCnoEmit/incrementalfalse andoldPWAactual
> manifest+4awaitedSharp metadata checks. References inpreinputs arefixedbefore57,
> not currentLayout; currentLayout1459 binds currentSource394/actualAST/fullafter.
> JSFS/.node readtrace1456rows/1437unique isnotOSloader or ESMselfattestation; old
> PWA47rows do notcapture libvipsPNGopens, fourphysicalPNG guards areseparate.
> Stale final-inventory labels, baseline1FAIL/two15+1genericFAIL, corrected historical
> Quest19 claim, Root reader boundaries/errors and reviewer probefailures remain
> retained; noneconverted to newpass. Real ReactDOM/hydration/concurrency/StrictMode
> andestranslation stillOPEN. [Independent report](C:/mir2-cross-platform-storage-20261002/repo-qa-pwa-selected-locale-01/source-review01/final01/report.md)
> 9447B/5364e5c6f64c21d27c2d509b82448b3d444bdbfa4e61d22968f3a7e708497e86 accepted.
> Root reclaimed both leases andstarted freshNext .next-pwa-selected-locale via
> realouter9ec963/liveexec97919; Next child/aggregatebuild acceptance stillfalse.
> Runtime4e exactM5reuse1711/dee14... isfrozen, no newRust/native/WASM build. Root
> archived allfour currentNextmetadata beforeinputs. LatercanonicalNFT/360MiBpackage/
> outsidecopy/Sharp/ownedHTTP/posttree/OS closure gates stillpending. GUI remains
> disallowed pendingexplicit consent afterEscape. Actualbrowser/native/device/
> PWAinstall/fullscreen/save/publicmedia/Candidate/goal completion stillfalse.
> The readonly nextQuestcopy inventory24inputs/fullarchives/28outputs isRoot
> accepted aspreparationonly (22614B/76fd3581090c33805e51aba696561a09bcc678caf032b1482ff3de3614fa8287).
> Sol/high prepares9path Return/Progress heading proposal using actual existing
> quest_presentation_text.rs andcanonical owner, noABI/DTO/protocol change; no
> proposedproduct writes/Cargo/imports authorized. Importer metadata/absolute
> source provenance requires explicitbounded generation, notwholebundle churn.
> Overallgoal remainsACTIVE, parentM5aggregate localbuild acceptance unchanged.


> M6 actual Next build is Root ACCEPTED (outer af8bab exit0; child close0/null,
> 58118ms). Current394 Source and27 runtime guards stay unchanged. Runtime4e/M5
> bytes are reused; no Rust/native/WASM rebuild. Only generated Next tsconfig two
> includes and next-env route path changed; tsbuildinfo and next.config remain
> exact. Four before/after metadata are archived outside394 Source.
> [Next acceptance](C:/mir2-cross-platform-storage-20261002/repo-qa-pwa-selected-locale-01/root-next-build-acceptance01.json)
> 95520B/041836547cc2b0de4052a059908bc6a63ab974e85af6c0b66afc3f564a6a0099.
> Strict canonical NFT audit outer916342 exit1 is RETAINED:32 files/61162 rows,
> 0 namespace/errors/uncertainties. Root three reader assumptions failed and remain
> diagnostic history. Exact full raw reconstruction and independent Sol/high review
> establish only30 same-prefix chunks249->989 sorted-block relocations, plus2
> three-backslash-config-row rotations;336 moved positions,2 rootNFT raw-identical.
> All other bytes/properties/multiplicity are preserved. Root96 metadata reads,
> independent206 current/archive inputs and109 outputs are actually verified.
> [Scoped NFT acceptance](C:/mir2-cross-platform-storage-20261002/repo-qa-pwa-selected-locale-01/root-canonical-nft-acceptance01.json)
> 144986B/a124ceaf918ba7eb1b5e84b281b6ebccb94bf164b3774d9f84adc54782a80a34
> accepts ONLY canonical metadata after reviewing real differences; no exit1
> relabeled0, no NFT dependency-target operations, no whole-dist coverage/status
> inheritance. M6 whole-dist audit remains unperformed, completenessfalse/exitnull.
> Root started fresh standalone package actual866d3a/live42424 with unchanged
> 360MiB cap, source-only receipt and exactM5 runtime reuse pin. Package/copy/Sharp/
> HTTP/posttree/owned OS closure and independent pipeline acceptance are pending.
> Single product writer is null; Root alone owns generated outputs/globaldocs.
> M7 proposal evidence20 fullfrozen/archive inputs,16 liveprotected product checks
> and29 outputs is accepted as preparation ONLY; no implementation or tests/build.
> [Proposal acceptance](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-quest-heading-locale-01/root-proposal-evidence-acceptance01.json)
> 26599B/48360471b7477e4ae17692c24e7311c42c9e5c7c21d652a4c1fc9bd23f84d083.
> Nine paths/two semantic headings/eight authored UI values are proposed; existing
> nonChinese OPEN Progress assertions may strengthen only to explicit newgoldens.
> Actual project-relative importer client JSON inputs exist and are fully archived;
> its two Build/Server/Debug inputs are absent. Discovered Server.MirForms source
> JSON is separately bound; Chinese766 matches, English766 lacks two committed768
> keys HeroDesummonCountdown/StoragePasswordCleared, other values unchanged.
> No normal/full importer ran, no source alias or unrelated dictionary repair is
> accepted. Root will bound new UI-key materialization while preserving every old
> token/metadata byte; full-importer smoke remainsOPEN. GUI consent afterEscape
> is still pending. Actual browser/native/device/install/fullscreen/save/public
> media/Candidate/goal-completion gates remainfalse; overallgoalACTIVE.
> Progress32: package actual51c8fc exit0 and isolated copy actualb9a655 exit0 completed.
> Package/copy beforeHTTP7301files/376406497B/776named directories/0links; 360MiB cap unchanged.
> Actual isolated Sharp d4b882 child57116 close0/null produced the expected PNG;42 JS/.node loads bind the copy. Runtime0.34.5 versus declaration0.35.3 retained; no OS-DLL/ESM closure claim.
> HTTP01 aa3912 exit1 is retained: new Page context contributes13 startup chunks versus prior12, so original specs31 plus manifest1 total32. Root accepted only six edit groups/seven occurrences in freshHTTP02; inverse recovers full old helper, unchanged lifecycle, ports, URLs and max32.
> HTTP02 actuald9461c exit0 performed32 real200 requests:27 SHA bindings+5 contracts, immutable M5 release bevy-4e3a46d90b6b8134, controlled local Prguse fixture only/public originfalse.
> All owned HTTP02 server49280 and fixture/helper60124 actual closes/log handles are complete; separate read-only OS receipts779ff5 and d74b09 confirm both owned PIDs and Sharp57116 absent, ports19160/19161 free. HTTP01 failed processes were independently closed too.
> Exact postHTTP tree7302files/376407218B/779named directories/0links preserves all7301 old files; only a721B optimizer PNG and three named cache directories added. Whole-tree-identicalfalse retained; no cache exclusions.
> Root source394+reference57 guards+runtime27 rechecked; four generated Next metadata files remain separately bound. No M6 Rust/native/WASM rebuild: runtime1711B/dee14eba... is exact M5 reuse, not a new binary acceptance.
> [Pipeline freeze](C:/mir2-cross-platform-storage-20261002/repo-qa-pwa-selected-locale-01/root-standalone-pipeline-frozen01.json) 1004043B/4c23eef59cd950b604a023ae4cc79f95b8073d3655b3f30b948f75a6cbe9713d binds1608 unique physical direct inputs and all actual phase receipts. Independent Sol high final LOCAL BUILD review is running; aggregate/build acceptance remainsfalse pending sealed review.
> Canonical NFT real916342 exit1 and reviewed generated chunk reorder remain retained scoped metadata acceptance only; M6 whole-dist audit unperformed/completenessfalse/exitnull. Neither missing whole-dist coverage nor old M5 failure is relabeled pass.
> [M7 bounded preparation acceptance](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-quest-heading-locale-01/root-bounded-generation-preparation-acceptance01.json) 48735B/b793d751ffebb883a064d44251fefb322ef38bfa46cf74aa6e0fdd47b9138f5f: Root full31 actual inputs+31 fullarchives,44 outputs,16 live protected sources and real Node executable are matched.
> M7 helper syntax-only c8e14f exit0 with empty stdout/stderr; .NET WaitForExit/Close are actual, not fabricated Node ChildProcess events. No AST/helper/materializer/generator/tests ran. Eight future authored headings must be AST literal-extracted under a fresh five-input Root license, exact eight JSON lines and inverse whole-buffer equality; existing actual subset builder generates Rust24keys/old22rows unchanged.
> Superseded full importer/VM/public-source aliases remain stopped; original server Build paths absent and historical two English server keys stay in frozen canonical unchanged. Nine product-path writer is still unassigned; M6 source frozen until independent aggregate acceptance.
> Model allocation: Root frontier architecture/integration, Sol high bounded implementation and independent review, Sol medium bounded verification helper work. No full-time MAX requirement. Actual browser/nativeGUI/device/install/fullscreen/save/public-media/Candidate/goal gates remainfalse; GUI Escape consent still pending and overallgoalACTIVE.
> Progress33: independent Sol high final pipeline review sealed and Root accepted aggregate LOCAL BUILD. [Aggregate receipt](C:/mir2-cross-platform-storage-20261002/repo-qa-pwa-selected-locale-01/root-aggregate-local-build-acceptance01.json) 5778B/551f76a6177fcb49f005fdfdb13371f3bf79bba5a705c3fabffbe37bb4bbc14a; [Root full independent proof](C:/mir2-cross-platform-storage-20261002/repo-qa-pwa-selected-locale-01/root-independent-standalone-pipeline-acceptance01.json) 9639162B/7a1379aee60e4882a35e4a7c354dfcc802ae8a5d99d28ae57e3bd3b0d517b89c.
> Root actually reread24162 unique files binding1642direct+14603physical current inputs/7903full independent archives/7916sealed outputs; all before/after maps and actual bytes match.32current canonical metadata are separately licensed beyond1608frozen direct; no dependency-target operations.19asset-metadata checks are actual, not20. Raw audit916342 exit1/HTTP01aa3912 exit1/whole-distunperformed/44warnings/Sharp0.34.5vs0.35.3/Root variable reader diagnostic and three independent reader assumptions remain retained.
> M6 accepted Source394/Next/scoped canonical metadata/package/copy/Sharp/localHTTP only; no M6 Rust/native/WASM build, exactM5 version4e reuse. Acceptance attaches to frozen M6 artifacts and does not make later M7 mutable source or binaries accepted.
> M7 Root captured394fullsources/57references/27runtime/4supplemental metadata before all product edits. [Before freeze](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-quest-heading-locale-01/root-before-frozen01.json) 249755B/d9a991ce6db071bc11006fca0f03c1a0bad038071ae23f34ad1f102208c3ce5e.
> [Formal9path grant](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-quest-heading-locale-01/root-implementation-grant01.json) 12136B/b9e9f4984d031a3e0716827261e3f5eaae06b42860a6d7fe8a5716c5ad338d52 and reviewedcontract7682B/ed7f561eb4a08802e76535bdd6e3906998afd8cca1e7f271b66f3ce050c19349 assign exactly9 existing product files to shared_quest_locale_impl Solhigh; other385Source/56currentreferences/runtime27/Next4 protected, only Webmirror reference licensed to change.
> Actual prior wrapper/physical inventory confirms Cargo cache rust-target/windows. Supplement1960B/8008cc9a79764fff55c64f673676811931e7c4bfeda7f23600ec3fd27ee4e162 corrects only targetpath; locked/offline/jobs1/stack64MiB and actualC>=50GiB before each Cargo remain. No simultaneous Root Cargo or runtime publication.
> Phase1 meaningful native/portable heading assertion BEFOREFAIL required with production22rows/threeold expressions untouched. New authored2keys/eightstrings then literal owners freeze for separate fresh five-input Root materializer execution license. Source grant does not run materializer/full importer/VM; actual subset builder must generate24rows and old22remain byte-identical. All final source/tests/new runtime acceptance pending.
> [Solmedium build preparation grant](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-quest-heading-locale-01/root-build-preparation-grant01.json) 29180B/660449a2539c80aa2d8cb8e73774876cdfe87efe77a5d8af453af7b6d86c7af7 binds53fixedinputs and separate QA-only directory; only syntaxchecks, no active source reads/Cargo/helpersemantic/build/network/GUI execution.11futurehelpers preserve caps and require newM7 canonical authority instead of exactM5reuse.
> Root owns docs/integration, one Solhigh code/Cargo writer, one Solmedium prep writer. Independent high review follows source freeze. GUI Escape consent still pending; browser/nativeGUI/device/install/fullscreen/save/publicmedia/Candidate/wholegoal remainfalse and goalACTIVE.
> Progress34: M7 shared Return/Progress headings now have actual semantic beforeFAIL in both native-ui and portable-quest-ui (three assertion failures each with old22 rows/old production arguments). Source work resumes under [Root sole-writer grant02](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-quest-heading-locale-01/root-implementation-resume-grant02.json) after prior agent loss; no simultaneous real Cargo.
> Literal owners are corrected inside actual esES/ptBR objects. The original misplaced-owner negative suite01 remains unaccepted, including its missing seventh independent child-event archive; no retrospective pass. Fresh positive02 produced equal737385-byte mirrors and seven actual expected-failure children closed1. [Root materializer review02](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-quest-heading-locale-01/root-materializer-artifact-acceptance02.json) 19954B/ad6629eaea8f3b6f39c48be570e4eee142cb9c3800c3741b49e3a149e68011a0 verifies eight authored lines/four independent locale goldens/full inverse736994 bytes/unchanged old keys-values-order-tags-metadata and actual Node22.18/TypeScript5.9.3 bindings. This is materializer evidence only.
> Pre-spawn runtime stop47430c is retained. Root repeated actual original historical F WebGPU full reads and independent PS/current pinnedNode checks; original30527815B/ca9 SHA and all old27 runtime guards currently match. [Revalidation01](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-quest-heading-locale-01/root-protected-runtime-revalidation01.json) retains causeunproven/original SHA/no F writes; recurrence still stops. A later actual builder code1 found malformed canonical mirror bytes near client.Statistics; full bad bytes retained, then only licensed canonical was recopied from the exact good candidate. Fresh actual subset builder03 passed0 and generated24 rows; old22 remain byte exact.
> Four required Rust runs passed nonzero11/6/11/6 and both adjacent portable filters1/1; four Node calls passed0. Three new compact test fixture failures remain realFAIL101: too-small simultaneous viewport, invalid tail top, and wrong active sheet. Corrected DetailReturn/DetailProgress/NpcReturn locale-current-stamp regression passed1/1, with explicit sheet identity/body probes/valid scroll/stale rejection. Final receipts and full394 Source/57refs/27runtime/Next4 freeze plus independent review are still pending; no Source acceptance or new runtime acceptance yet.
> [Root accepted build preparation only](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-quest-heading-locale-01/root-build-preparation-acceptance01.json) 67355B/61ce894fca78915b07351fba6df69ddee1c1491b89cb1e561a5cf0cb41e4c5bd verifies54fixed inputs/111outputs/six exact forward-and-inverse derivations/four exact copies;9Node syntax exits0/two PS parses0 are not semantic execution. Native draft and three-renderer canonical draft require separately accepted final M7 Source and new canonical authority; canonical stays PENDING. Next supplemental metadata changes must be reviewed explicitly before later gates.
> [Per-call Cargo guard plan](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-quest-heading-locale-01/root-cargo-per-call-plan-acceptance01.json) is accepted preparation only:44outputs/six inputs with full before-after archives, original partial14file history and actual API reader diagnostics preserved. Root chooses exact child exit with mandatory independent completion-evidence audit. A Solhigh worker is separately licensed for QA-only ConsoleApplication implementation/compilation/controlled fixtures; no real Rust Cargo/product build under that guard grant. Existing CARGO_BIN and original lexical cargo.exe dispatch, actual freshC>=50GiB before every version/build/retry, raw argv/cwd/env/stdhandles and receipts remain required.
> Root leads architecture/integration and owns global docs; Solhigh owns one bounded source/Cargo lane and one disjoint QA guard lane, independent high Source review follows freeze. Prior Solmedium build preparation is sealed. GUI Escape consent remains pending; no browser/nativeGUI/device/install/fullscreen/save/publicmedia/Candidate/wholegoal acceptance, overall goalACTIVE.
> 2026-10-02 UTC progress35: M7 shared quest Return/Progress headings SOURCE
> is now Root accepted after independent review (no actionable P0/P1/P2).
> Exactly9 allowed sources changed;385 protected sources remain unchanged.
> Final37 Rust cases,5 presentation tests,4 Bevy-locale tests,22 current V2
> definition checks and the24-row generator pass; all previous22 rows survive.
> This is source/test acceptance. It does not inherit M5/M6 running-client proof.
> [Root source acceptance](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-quest-heading-locale-01/root-source-acceptance01.json).
>
> The QA-only per-Cargo guard has29 actual controlled fixture passes and an
> independent static review with0 P0/P1/P2. Root verified686 artifact pins and9
> public tools, then issued separately pinned real-Cargo authority. First actual
> guarded cargo1.95.0 --version finished with actual spawn/exit/close0 and one
> fresh Get-Volume C before/after receipt. Policy B still rejects missing after
> evidence. Windows native-build02 is now actually running, not yet accepted;
> the unchanged repository three-package canonical build is next in the sole
> Root Cargo lane. Latest Next/package/HTTP/client gates remain pending.
> [Execution authority](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-quest-heading-locale-01/root-real-cargo-build-execution-grant01.json).
>
> Next metadata helpers have bounded Root-reviewed preparation: original before4
> pins; only two tsconfig include appends and one next-env routes-import change;
> every other metadata/source/reference/runtime byte remains guarded. Syntax
> checks are preparation only. Actual Next/runtime/package results must be read
> before acceptance. Original failures and reconstruction limitations remain.
>
> User requested visible playable progress after two days. Work is narrowed to
> fresh runnable clients and player-flow proof; review/helper completion alone
> is not parity. Full HUD/compositor/combat/social/settings/device work remains
> OPEN. Escape GUI reauthorization is still pending; no browser/native GUI was
> resumed. Actual-client/device/save/public-media/Candidate/goal remain false.

> Progress36: latest M7 Windows executable has actual guarded build exit/close0
> in544s; all482 source/reference/runtime/Next guard rows match before/after.
> Root accepts the compiled103572480-byte executable as build only; it was
> never launched. The original three-package canonical release build is now
> actually running (webgpu stage), sole Root lane. Next/package/HTTP/GUI remain
> pending. [Actual current build status](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-quest-heading-locale-01/root-live-build-status02.json).
> Candidate/goal/client/device acceptance stays false.

> Progress37 (2026-10-02): M7 local builds and standalone startup are complete:
> Windows executable, all three WASM packages, Next, isolated 359 MiB package,
> actual Sharp loading and 32/32 HTTP requests passed. No latest GUI/gameplay,
> device, save or public-origin acceptance is implied.
> [Build evidence](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-quest-heading-locale-01/root-aggregate-local-build-acceptance02.json).
> M8 now implements the shared main HUD, authoritative Character Stats and
> Character/Bag/Quest navigation. Sol xhigh is the sole Source/Cargo-test writer;
> Root leads architecture and integration. Full combat/social/settings/devices
> and Candidate remain OPEN.
> [M8 scope](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-hud-stats-01/root-implementation-grant02.json).

> [Cross-platform Progress38 / 2026-10-02] M8 shared main HUD, authoritative Stats1/Stats2/header and Character/Bag/Quest navigation have Root SOURCE + focused-test acceptance: 28 changed/new paths, 406 frozen Source and 371 protected originals. Actual Native6/Portable6/Runtime5/ABI2/Node21 pass; focused WASM and nonincremental TSC exit0. Independent bounded ingress review has 0 remaining P0/P1/P2 and nine final input hashes matched. Root owns the Windows and three-runtime build lane; Windows build is running, with no new build accepted yet. M7 local artifacts remain prior evidence. Latest GUI/gameplay, device/save, full HUD/compositor, compatibility menus/economy, 100% Candidate and goal remain OPEN. Evidence: C:/mir2-cross-platform-storage-20261002/repo-qa-shared-hud-stats-01/root-source-acceptance01.json.

> [Cross-platform Progress39 / 2026-10-02] M8 shared main HUD and Stats1/Stats2 have accepted Source/focused tests, Windows binary, three canonical runtimes (bevy-bcfaf1f4d568d1f5), Next and isolated standalone startup. Package376,804,095 bytes/7,301 files/zero links is under360MiB; actual Sharp loads42 files inside the copy. Actual local HTTP32/32 and19 metadata checks pass; both owned PIDs/ports are absent. All original copy hashes remain exact, plus one721-byte optimizer cache. Three media images use local fixtures; public delivery and current GUI/gameplay/device/save/Candidate/whole goal remain OPEN. M9 shared Character paperdoll/14 equipment slots/inspect/remove is authorized to one Sol-high Source writer across29 paths, reusing the current controller/ordinary RemoveItem ACK; no implementation acceptance yet. Evidence: C:/mir2-cross-platform-storage-20261002/repo-qa-shared-hud-stats-01/root-aggregate-local-build-acceptance01.json.

> [Cross-platform Progress40 / 2026-10-03] M9 shared Character paperdoll and 14 equipment cells/inspect/remove have Root Source and focused-test acceptance across30 authorized changed/new paths:421 declared Source,391 protected originals and one current-only item-identity dependency. Root42 Rust and39 Web regressions, actual Host-to-Router pointer deserialization, focused WASM check and prior unchanged-Web TSC pass. Two real compile errors were repaired; independent original/delta reviews have0 remaining actionable P0/P1/P2, with missed compile checks and failed histories preserved. Old Quest/Bag/HUD ABIs stay exact. Root owns the Windows build now running, followed by three canonical runtimes, Next and package checks; no M9 binary/package acceptance yet. Spells/hotkeys next boundary retains7 learned rows/16 keys and existing request-scoped receipt semantics, but has no implementation acceptance. Latest GUI/gameplay, live equipment ACK, resource delivery, mobile layout/devices/save, economy/settings/recovery,100% Candidate and whole goal remain OPEN. Evidence: C:/mir2-cross-platform-storage-20261002/repo-qa-shared-character-equipment-01/root-source-acceptance01.json.

> [Cross-platform Progress41 / 2026-10-03] M9 shared Character/equipment now has Root native build acceptance: actual guarded Cargo exit0 and child close0, 517 source/reference/runtime/Next guards unchanged, and an exact unlaunched Windows EXE. The interrupted native01 attempt remains unaccepted; retry02 provides the complete exit and executable evidence. The original three-backend canonical Web build is actually running under the sole Root lane; M9 Web/Next/package acceptance is still pending. For the user-facing Windows/Web/future-mobile consistency and reuse goal, approximately35% (30–40%) is an engineering estimate, not a measured Crystal parity or Candidate percentage. Shared quest/bag/HUD/EXP/weight and this Character slice are bounded progress; skills/combat/economy/settings/recovery, current gameplay/resources/save and physical Android/iOS acceptance remain OPEN. M10 must fence HMR request IDs and validate local id plus explicit spell before pending projection; these two safeguards are identified but not implemented. Evidence: C:/mir2-cross-platform-storage-20261002/repo-qa-shared-character-equipment-01/root-native-build-acceptance01.json.

> [Cross-platform Progress42 / 2026-10-03] M9 shared Character/equipment has Root bounded Source/tests, Windows build, three canonical Web runtimes (bevy-8f5daa54ba368731), Next and isolated standalone startup acceptance. Package377,397,312 bytes/7,301 files/zero links passes360MiB with90,048-byte margin. Actual COPY02 preserves every file; native Sharp imports/resizes using42 measured in-copy dependencies, distinct from21 physical Sharp/native package files. Actual local HTTP32/32 and19 metadata checks pass; all7,301 original copy hashes remain exact with one721-byte optimizer cache, final966 Source/runtime/Next checks pass, and owned PIDs/ports are absent. Interrupted native01, failed copy01 and QA inspection/runner mistakes remain retained. Two media images are copied and three use local fixtures; public delivery and current native/browser gameplay, live equipment ACK, save, mobile layout/devices and100% Candidate remain OPEN. M10 shared Spells/key assignment is the next implementation, with7 learned rows,16 keys, one Rust state/painter, burned HMR request namespaces and exact owner/id/spell receipt/projection validation; no M10 implementation acceptance yet. Whole-goal approximately35% (30–40%) remains an engineering estimate, not measured Crystal parity. Evidence: C:/mir2-cross-platform-storage-20261002/repo-qa-shared-character-equipment-01/root-aggregate-local-build-acceptance01.json.

> [Cross-platform Progress43 / 2026-10-03] M10 shared Spells state and common Native/portable painter have Root preliminary compile-check acceptance: actual non-test Windows Cargo check exit0/close0 and wasm-release/webgl2-shared-ui Cargo check exit0/close0, with985 before/after guards matching in each run. The partial426-Source candidate contains6 changed originals,4 new files and416 protected originals;11 new integration files were unapplied at that historical freeze. Native02 actual E0425/E0308 and incomplete clipping review are retained; repairs passed the actual checks and independent core static review03 has0 remaining P0/P1/P2. Common outcome-unknown requests wait for exact receipts without projecting key changes; ordinary text clipping and the existing centered assignment prompt policy are restored. The sole Sol-high Source writer has resumed the remaining portable/runtime/Web implementation within the original29-path grant. Complete M10 Source/tests/review, production binaries/runtimes/Next/package, current native/browser gameplay, live skill ACK, resources/save, mobile layout/devices and100% Candidate remain OPEN. Whole-goal approximately35% (30–40%) remains an engineering estimate; the complete goal stays active. Evidence: C:/mir2-cross-platform-storage-20261002/repo-qa-shared-spells-keys-01/root-core-checks-acceptance01.json.

> [Cross-platform Progress44 / 2026-10-03] M10 shared Spells is now Root-accepted at the complete 437-file Source and focused-test level. Its nine actual Cargo jobs recorded 126 passing Rust test executions (15 portable, 67 Native, 32 Host, 5 independent ABI, 5 HUD-ingest and 2 explicitly invoked JS-captured DTO tests; repeated executions are not unique tests), plus Node 24/24, current setter capture 13/13, unchanged compatibility 39/39, nonincremental TSC exit 0, and Native/shared-WASM checks exit 0. Independent full01 and delta02 reviews report zero remaining P0/P1/P2 findings; all 1,011 before/after guards matched on each job. M11 now implements the authorized complete shared ordinary skill-and-melee rules within 17 paths (10 existing, 7 new), against 437 starting Source files and 66 references/41 runtime/4 Next metadata inputs; its implementation is not yet accepted. M10 and M11 production Native, all three canonical runtimes, Next and package will be validated together once, with no separate M10 production pass claimed. Production builds, current GUI/native/browser play, live skill ACK, save, public resource delivery, mobile/device behavior, Candidate and whole goal remain OPEN. Following user criticism, prioritize complete end-to-end play and parity evidence; do not publish another percentage estimate. Evidence: C:/mir2-cross-platform-storage-20261002/repo-qa-shared-spells-keys-01/root-source-acceptance01.json; C:/mir2-cross-platform-storage-20261002/repo-qa-shared-combat-input-01/ROOT-CONTRACT.md.

> [Cross-platform Progress45 / 2026-10-03] M11 is Root-accepted at the Source and focused-test level: 444 Source files (17-path delta: 10 existing and 7 new; 427 protected starting files; 18 allowed rows including the extra cfg(test) capture compile fix), 66 references, 41 runtime files, 4 Next metadata files, and zero supplemental current-only inputs. Nine actual Cargo jobs exited and closed 0 with all 1,089 before/after guards matching per job. Focused Rust execution count is 388 (Native common 83, portable 31, Native adapter 235, Host/ABI 38, plus one actual-JS-snapshot/edge Rust consumer); these are executions across configurations, not unique tests. Three default-ignored tests remain uncounted; the M11 consumer was explicitly run with --ignored. Four native/three-WASM compile checks passed; Node 39 Root-run whole-JS/AST/router checks passed; nonincremental TSC exited 0. Static full01 and delta02–05 reviews have zero remaining P0/P1/P2 findings. M11 production Native, canonical runtimes, Next and package are not yet accepted and remain planned with M10 as one combined batch. Production artifacts, current GUI/native/browser gameplay, live ACK/save, Native queue/socket isolation, public resource delivery, Android/iOS real-device behavior, Candidate and whole-goal acceptance remain OPEN. Following user criticism, prioritize complete end-to-end play and parity evidence; no percentage estimate is reported. Evidence: C:/mir2-cross-platform-storage-20261002/repo-qa-shared-combat-input-01/root-source-acceptance01.json (18,368 B; SHA-256 0fb94513f6b2d41e4012ee72ec7f5ad5326d69b9969971acc85286ba9a0ed1bf); source snapshot root-source-snapshot01.json (247,041 B; SHA-256 4dd10c12a4786b5127632715315226d5e277be3598bd54d64152a5d7984a58ac).

> [Cross-platform Progress46 / 2026-10-03] The combined M10+M11 local Native, all three canonical WASM runtime and Next builds, independent package copy and HTTP checks are Root-technically accepted at runtime `bevy-49c4a8559bee071c`; this is local build/HTTP evidence, not GUI or gameplay acceptance. Focused compile checks were four total: one Native and three WASM. The original 360 MiB package attempt exited 1 and is preserved; only the assembled package’s generated respawn JSON received authorized whitespace-only compaction, after which the package passed at 375,633,540 B / 7,301 files / 776 directories / 0 links under the unchanged 377,487,360 B cap. Isolated-copy verification checked all file hashes, Sharp 42 closure, 32 declared HTTP responses, 19 metadata responses, 2 copies and 3 fixtures; post-boot the original package retained its 7,301 files, with only 721 B / 1 file / 3 directories of boot output, and owned HTTP PIDs/ports exited. The current compiled consumer read the original or compact manifest six times per run across map `0` and `D011`; complete blueprints matched, including respawn fallback centers `(350,200)` and `(200,200)`. Native transport has a minimal design prepared but remains unimplemented. Production GUI, complete gameplay, live ACK/save, public assets, Android/iOS devices, Candidate and whole-goal acceptance remain OPEN. GUI restoration was requested from the human and is not authorized; do not operate it without new authorization. No percentage estimate is reported. Evidence: [shared Spells and combat QA](generated/player-qa/client-core-20260930/shared-spells-combat/README.md); combined build acceptance: `root-aggregate-local-build-acceptance01.json` (7,975 B, SHA-256 `8043b195c098a08aef6d1f8524234d326c63e598e94a6b9d68f53fe56b223a16`).

> [Cross-platform Progress47 / 2026-10-03] M12 Native command ownership and final socket fence have Root Source, regression and fresh EXE build acceptance: 444 Source files, 6 changed originals and 438 protected; 66 references, 48 runtime files and 4 Next metadata preserved. Final actual Native check passed and bin regression passed 377/377 including all 18 ownership tests, with 1,091 guards per job; final independent review has zero confirmed P0/P1/P2 findings. Five bounded fixes cover exact unsent storage cancellation, packet-first full-world/Skill identity, canceled-entry late flush, unsent personal-owner reset, and valid pending retention across empty frames while awaiting Skill. Real server full snapshots without mapIndex retain raw mapFileName compatibility. One actual Native build exited/closed 0; .NET child Exited/Disposed are true under Policy B, 562 build guards match and child/guard/probe PIDs are absent. EXE is 104,005,120 B, SHA-256 d4be4c8c1a6cbe0c4beced90bde5914ee2163b3e7cf2a425bb59c926c8bde0a3; it was not launched. Original test compile101 (zero executions), output-directory preflight failure, 376/1 invalid fixture and subsequent 376/1 genuine pending defect remain retained before final Source06 pass. No WASM/Next/package rebuild was performed for this Native-only delta; M11 Web artifacts remain prior evidence. Latest human direction is continue code and temporarily do not operate UI; no GUI operation is authorized. Same-filename protocol ambiguity and initial connection-cancellation scope remain open; live play/ACK/save, public resources, mobile devices, Candidate and whole goal remain OPEN. M13 shared Mail list/reader common Native/portable painter and single Web owner/handoff are next, not yet accepted. No percentage estimate. Evidence: [M12 QA](generated/player-qa/client-core-20260930/native-command-ownership/README.md), root-source-acceptance01.json (13,279 B;75bda24da8dd9f1da140d18a4c5e195711a037d9e06cb9ab109d37faa9722824), root-native-build-acceptance01.json (4,797 B;1f047805b481329c56128717c7225ec6dde3852209fdb9a270c4f6a2d4a2425c).

> [Cross-platform Progress48 / 2026-10-03] M13 shared Mail list and letter/parcel readers now use one Rust Native/portable painter with sole Web ownership and acknowledged retirement before DOM fallback. Frozen Source08: 455 source files, 19 changed originals, seven additions and 429 protected originals. Root accepts eight focused Rust jobs (175 passing executions, not unique tests), fresh Windows EXE, three canonical WASM builds, 46 compiled exports and Next. The exact Source04 Web tests carry 22 Mail + 118 related passes and TypeScript exit0; Source08 did not regenerate Node fixtures. Fresh pure WASM consumers compare all21 original Node captures on three backends: 63 full parsed outputs equal and15 negative inputs reject; no boot or graphics. Runtime bevy-b070e88b10ce12c4 shared WASM is32,499,664 B, 6,192 B below the unchanged cap. The original thin-package build exited1 at380,426,871 B; preserved package-only whitespace compaction of two generated JSONs produces377,468,964 B, 18,396 B below the fixed360 MiB cap. All7,301 files/776 directories/zero links are verified; only two JSON files changed and7,299 files stayed exact. Two complete isolated copies pass18 actual HTTP responses (14 full JSON +4 exact WebP) and8 direct compiled-consumer calls; real0/D011 null-center fallback blueprints match explicit manifest centers. HTTP omitted-center8,8 normalization stays separate; literalN16 is a diagnostic with no map cells, not public quest-content acceptance. After execution, all original/copy trees and1,105 physical input pins (1,096 source/runtime/Next guard rows,22 metadata-only aliases) match; Root observed all six owned helper/server/probe PIDs absent and22741/22742 free. Original failed builds/preflights are retained. Latest human direction remains continue code, temporarily no UI. Shared compose/editor/IME/stamp/attachments/send, actual GUI/play/live ACK/save/relogin/public resources/mobile devices/Candidate and overall goal remain OPEN; no percentage estimate. Evidence: [M13 QA](generated/player-qa/client-core-20260930/shared-mail-inbox/README.md), root-reviewed-package01.json (8,622 B; f1a176cdf85ccd0ec15311cd8dbc24b3f05ae9aad3e384c108e8293f8ab0fbcc).

> [Cross-platform Progress49 / 2026-10-03] M14a common compose validation and actual Native/Web send consumers have Root SOURCE + focused-test acceptance on Source04:462 pinned source files,18 changed existing files,two new modules and442 protected existing files. Native reducer/actual Submit and compatibility Web Page call the same dependency-free Rust500 UTF16 whole-reject/normalization/trim/gold/ordered-UID policy via the independent small-WASM edge; Web draft/single-flight/exact final socket proof/failure retention/success ACK and same-connection retired-owner ACK slot are covered. Actual Node36/36, six Rust test jobs130 passing executions (not unique tests), explicit compiled replay of nine actual Page/wrapper captures, Native check and nonincremental TypeScript exit0 pass. Source03 Node/core/strict-host/replay consumers stay byte-exact on Source04; they are carried, not regenerated. Original reducer startup101/zero executions is preserved; adding only client-core to root workspace exclude repairs its independent workspace boundary. All1091/1093/1094 per-job guards match and Root observed all20 known child/probe PIDs absent. Final independent reviews confirm no remaining P0/P1/P2. This accepts source/focused checks only: fresh Native EXE/small-WASM/three canonical/Next/package production builds will run together after M14b parcel/quote and M14c common painter/text adapter; unchanged caps and prior M13 artifacts remain separate evidence. Latest human direction remains code only, no UI. Actual GUI/play/live ACK/save/relogin/public resources/mobile devices/Candidate and whole goal remain OPEN; no percentage. Evidence: [M14a QA](generated/player-qa/client-core-20260930/shared-mail-compose/README.md), root-source-acceptance01.json (12,010 B;604b6552026e96e37dfd05671350dddc7b0135d914ec92e9a1e7451facdbf306).

> [Cross-platform Progress50 / 2026-10-03] M14b shared parcel selection/lock/stamp/postage has bounded Root SOURCE + focused acceptance on frozen Source05:465 source files,14 changed originals,3 additions and448 protected originals. Fresh Web Mail52/52, portable26/26, explicit compiled parcel replay42 sessions/294 full outputs and compose replay11 actual captures pass; exact unchanged consumers carry Native100, core7, strict-host5 (2 default ignores), adjacent36 and nonincremental TypeScript exit0. Six Rust test jobs total140 passing executions, not unique tests. All per-job1102/1105/1106/1107/1113 guards match; known child/probe absence and independent zero confirmed P0/P1/P2 are recorded. Original Source01 failures, two51/52 test-harness failures and unexecuted Source02 preparations remain retained. Native true connection epoch through buffer/inbox/UI and exact quote socket-entry receipts are still OPEN: local FIFO acceptance does not prove transmission, and same-connection DataReset must preserve late Cost. Next is bounded Native transport code, then M14c common composer/editor/IME and combined fresh production builds under unchanged caps. No UI operated; gameplay/live ACK/save/relogin/public delivery/mobile/Candidate/full goal remain OPEN. Evidence: [M14b QA](generated/player-qa/client-core-20260930/shared-mail-parcel/README.md), root-source-acceptance01.json (16,858 B;4406ba1c07538f414e89733f5bf0985e3f660be8c1b779cbb3f7f87f8b4b5af6).

> [Cross-platform Progress51 / 2026-10-03] Bounded Native Mail stream/reset/marker SOURCE + focused acceptance is complete on Source04:467 declared existing inputs,9 changed product files and458 protected inputs. Trusted positive (run,connection) is captured before packet delivery and survives buffer/inbox/UI; current-stream Cost survives all three reset layers. ACK-safe marker byte admission fixes the independently confirmed P2. Native103 and production check carry by exact consumed inputs; fresh runtime16/32, portable27, shared-WASM check, Windows802 (3 default ignores), platform-web host5 (2 default ignores), and explicit parcel/compose compiled replay each pass. Old fixtures supply42 actual Web sessions/294 full outputs and11 actual Page/wrapper captures; no new JS producer run is claimed. Earlier platform-web Cargo.toml/lock content was absent from the465 freeze, so Progress50 old host/replay pins are historical only; Source04 explicitly pins both and fresh host/replays close current inputs. Original pre-spawn, two runtime compile failures and Windows PATH failure remain retained. All83 references/73 runtime/4 Next metadata/22 aliases match;26 known owned children actually exited/closed and final snapshot is empty. Independent review reports zero remaining confirmed P0/P1/P2 for this scope. Exact quote socket entry and old MailSent ACK remain OPEN; next is one12-path quote-ticket writer, then typed send ACK, M14c common painter/editor/IME and combined fresh production builds under unchanged caps. Code only; no UI, production-build, live gameplay/save/relogin/public/mobile/Candidate or goal acceptance. Evidence: [Native stream QA](generated/player-qa/client-core-20260930/native-mail-stream/README.md), root-source-acceptance01.json (18,322 B;ed0c3a40571fc267c0e36e7d564b40d9aad85e4d2c1dc6a76b990bbce35aa292).

> [Cross-platform Progress52 / 2026-10-04] Native Mail quote-ticket SOURCE + focused checks are accepted on Source04:467 existing inputs,12 changed files and455 protected inputs. Exact token/ticket binding precedes publication; timeout starts at actual start_send entry, and definitely-unsent retirement is identity-bound. Receipt/Cost order and tombstones survive all three same-stream reset layers; true stream change and poison handling fail closed. Core11, Native110, runtime Mail23/ingest36, portable30, Windows812 (3 default ignores), Web host5 (2 default ignores), shared-WASM type check and two explicit compiled replays pass:1029 Rust test executions, not unique scenarios. Seven earlier positive jobs carry by exact consumed inputs; three real fixture/compile failures are retained. All467 current/final/before archives,84 references,73 runtime pins,4 Next metadata and22 aliases match; all28 known Cargo/probe children exited/closed and are absent. Independent review reports zero remaining confirmed P0/P1/P2 for this bounded scope. Controlled production Sink entry is not full socket/client evidence; dedicated quote-receipt byte-refusal coverage and interruption of permanently blocked flush are not claimed. Next:17-path Native typed MailSent flight/complete-draft ABA protection, then M14c shared composer/editor/IME and combined production builds within unchanged caps. Code only; actual clients/mobile/Candidate and goal stay open. Evidence: [Native quote QA](generated/player-qa/client-core-20260930/native-mail-quote-ticket/README.md), root-source-acceptance01.json (21,941 B;a8d9e8d49af4dbf69f0c79aabce8a8312b550511ab114a0d9fd48f6820b7674a).

> [Cross-platform Progress53 / 2026-10-04] Native Mail send-flight SOURCE + focused checks are accepted on Source05:467 existing inputs,17 changed files and450 protected inputs. Shared Core owns one checked send token and persistent full-raw draft generation; actual queue admission, prepublish ticket binding and irreversible start_send entry preserve exact unsent/entered/unknown outcomes. Legal 1/-1 settles only the old transport and Pending binding; draft/incarnation/raw generation separately gate UI effects, including reducer/editor/UID/stamp ABA. Critical FIFO and tombstones survive all three same-stream reset layers. Eleven actual positive jobs cover Core13, Native121, runtime Mail28/ingest40, portable31, Windows820 (3 default ignores), Web host5 (2 default ignores), Native/shared-WASM checks and two explicit compiled replays:1060 Rust test executions, not unique scenarios. Seven exact-input qualified jobs carry and four are fresh on Source05; two actual fixture/compile failures are retained. All26 full authored test names match passing raw stdout. Current/final/before source,86 references,73 runtime pins,4 Next metadata and22 actual alias realpaths match. All26 known job/probe children exited/closed and were absent at the recorded snapshot. Windows was serial --test-threads=1; default parallel queue safety is not claimed. Independent reviews report zero remaining Native P0/P1/P2. Controlled channels/Sinks and pure App are code evidence; permanently blocked poll_flush interruption, actual connected clients/mobile and new production artifacts remain open. Evidence: [Native send QA](generated/player-qa/client-core-20260930/native-mail-send-flight/README.md), Root acceptance C:/mir2-cross-platform-storage-20261002/repo-qa-native-mail-sent-01/root-source-acceptance01.json.

> [Cross-platform Progress54 / 2026-10-04] The Web arbitrary-integer MailSent P1 is closed in bounded SOURCE + focused checks:only primitive numeric 1/-1 may retire an entered send barrier, including ownerless old ACKs. Source01 changes two existing files against Native Source05 and preserves465 other inputs; both guard lines and the5698-byte additive actual-Page AST test block reverse exactly, retaining every prior assertion. Fresh Node55/55, nonincremental TypeScript and both current compiled exact ignored Rust replays pass (one test each). The actual producer emits19 compose captures and47 parcel sessions/361 outputs; these controlled captures are bound to replay evidence. All467 current/final/before source rows,86 references,73 runtime pins,4 Next metadata and22 alias realpaths match; all8 known finite-check/probe children exited/closed and were absent at the recorded snapshot. No browser/socket/client was operated. This closes the malformed-result defect only; M14c1 stable pure-Core send-slot ABI/facade and M14c2 shared composer/editor/IME/painter remain next, followed by matched production artifacts within unchanged size caps. Code-only authorization remains; actual login/combat/save/relogin, device/frontend/Candidate acceptance and the goal stay open. Evidence: [Web ACK QA](generated/player-qa/client-core-20260930/web-mail-ack/README.md).

> [Sealed707 evidence](generated/player-qa/client-core-20260930/shared-mp-orb-dpr2/README.md) ·
> [Native failed entry/retry boundary](generated/player-qa/client-core-20260930/shared-mp-native-entry/README.md) ·
> [Shared EXP contract](architecture/shared-experience-bar-host-contract.md) ·
> [Shared weight contract](architecture/shared-weight-bar-host-contract.md).

> 2026-10-01 HP retry5a7c9a closes the bounded Rust image-request defect.
> Portable8/8, nativeHUD38/38, runtime10/10, native/three-WASM checks, canonical
> artifacts, actual Next and a 376,190,199-byte/7,300-file/zero-link clean copy
> pass the unchanged budgets. Actual sharedGL2 fault01 has one abort over
> 231,234ms at generation2; same-page Login generation4 allows one new abort
> over114,653ms. Ordinary reload recovers HP; GPU secondary UI, lean React and
> process-DPR2 touch also work. Five fresh runtime hash cases plus one same-App
> reuse,70 retained PNGs,10 movement ACKs and six normal Logout/save revisions
> 248–258 match246 across11 public fields. All owned services/listeners exited;
> shutdown store bytes match. Root364-source/35-helper/86-audit and7,300 copied
> file checks pass. Retained driver mistakes and corrective core audit v2 stay
> explicit. Native physical/Welcome/trusted capture/Logout, partial HP, phones,
> full HUD/composition, performance, public delivery and human acceptance stay
> open. Shared MP is selected by read-only audit, pending a separate code lease.
> Root owns architecture/integration/actual clients, Sol/high bounded code and
> independent review, Luna/medium fixed evidence. Overall goal remains active.
> [Retry results](generated/player-qa/client-core-20260930/shared-hp-orb-retry/README.md) ·
> [Original HP evidence](generated/player-qa/client-core-20260930/shared-hp-orb/README.md) ·
> [Retry contract](architecture/shared-hp-orb-retry-contract.md).

> 2026-10-01 portable thin03 closes the bounded local packaging gate:
> 358.65 MiB, 7,300 files, zero links and an independently booted Windows x64
> native dependency closure. Page imports use the same 198-key derived index;
> output-only JSON compaction and inventoried remote media retain source data.
> 107 cold requests and shared GL2 touch-emulation/GPU mouse/lean mouse
> Bag/Diary/movement/Logout regressions pass; saves 215/217/219 equal 213.
> All 29 PNGs retain 24 passing and five initial/failure captures, with four
> driver mistakes explicit. Source/package/helper and cleanup hashes match;
> independent evidence audit is complete and root-reviewed, including 322
> indexed source/evidence hashes at closeout. Shared Diary applies Main/active
> eligibility and priority sorting; React offers all/active/completed filtering.
> Their one/four counts do not prove state loss, and shared tracking/locales
> still differ. Full Diary consistency is not accepted by opening both panels.
> Configured proxy samples are loopback; release-manifest 404 and zero old
> Pet/Gate manifest coverage leave public media delivery open. Phones, DPR2/IME/
> safe areas, mixed input, loading/memory, all HUD/surfaces, world/UI composition
> and CP-02–04 stay open. The next bounded slice shares HP orb painting and
> retains existing controls/text until their own replacements are verified.
> [Results, retained failures and limits](generated/player-qa/client-core-20260930/portable-thin/README.md).

> 2026-10-01 f506 bounded input repair is built and browser-verified: lean GL2
> and GPU primary mouse movement has compatibility events and position ACKs;
> shared GL2 touch directions and active-to-null Stop pass eight center-held
> gestures without extra requests. Touch-game PWA is hidden at600/640/844 and
> normal Login restore retains hit-tested install/fullscreen controls. Shared
> Bag/Diary entry and GPU secondary UI close have58 PNGs across four cases and
> normal saves207/209/211/213 matching205. Audits are root-reviewed. Case02 lost
> old response bodies; fresh case03 hashes its own startup. Closed early Diary
> captures and incorrect hidden-canvas gates are retained QA mistakes, not
> blank-render/Close product failures. Current work is a separate portable thin
> copy/dependency slice; previous10.94GB linked output stays failed. Physical
> mobile, locale/HUD, mixed input, DPR2/IME/safe areas, compositor/performance
> and CP-02–04 remain open.
> [Results and limits](generated/player-qa/client-core-20260930/web-input-routing/README.md).

> 2026-10-01 selectable49d3 now has ordinary lean/shared/realGPU core panels,
> current response hashes, normal saves and controlled transport-fault DOM/reload
> recovery.57 PNGs include realGPU844 More/PWA failure and retained harness
> failures;25 earlier failed/restoration captures stay separate. Independent
> Luna/medium audit is complete and root-reviewed. Single-run startup/cache
> observations are not performance acceptance. Primary world canvas mouse,
> joystick Y and PWA ownership are the current bounded repair; active→null
> joystick intent must also clear Page's retained movement through ordinary Stop.
> The thin app exceeds10.9GB with external junctions and remains nonportable.
> Full locale/HUD, devices, mixed-input desktops, DPR2/IME/safe-area, composition
> and CP-02–04 stay open. [Current results](generated/player-qa/client-core-20260930/webgl2-package-selection/README.md) ·
> [Input contract](architecture/web-input-routing-contract.md).

> 2026-10-01 controlled default/shared GL2 artifacts now differ by10,006,769 raw/
>593,642gzip bytes with equal measured exports and read-only ABI1 checks. This
> reduces compile reachability; the current95e4 publisher still serves its
> UI-capable GL2 package to ordinary pages. Default React/sharedGL2/GPU have
> separate31-PNG ordinary UI/save regressions at documented sizes. Actual package
> selection, complete three-package release/rewrites/failure recovery and measured
> client startup/memory remain open. Full locale/HUD/devices/compositor gates are
> unchanged. [Current weight scope](generated/player-qa/client-core-20260930/webgl2-weight-pair/README.md).

> 2026-10-01 experimental f7 shared Quest is readable at actual600×320 in
> desktop touch emulation: Diary, full N3 Text reading, fixed Rewards, Cancel,
> current measured560×296 CSS layout and save/relogin.640/844 and default GL2/GPU
> regressions pass their bounded gates. ba90's actual-scale panelBounds failure
> is retained and reproduced; only compact subtree rounding is disabled.
> English entry currently opens Chinese shared captions/body, so full locale
> consistency remains open. NPC/multiple reward/Finish ACK/long confirmation
> are fixture checks in this round; physical devices, DPR2/IME/safe-area, all HUD,
> world/effects composition and payload/performance remain unresolved.
> [Current evidence and limits](generated/player-qa/client-core-20260930/compact-quest/README.md).

> 2026-10-01 mobile More Diary entry is implemented and has bounded actual
> English/Chinese/Portuguese600/640/844 touch-emulation evidence.40/44px panel
> targets and PWA spacing avoid the retained entry/close obstructions; opening
> Diary also dismisses an open touch system menu.600 still has a tiny React
> Diary and needs a readable shared compact layout. Full shared-window locale,
> HTML lang for non-English UI, whole HUD, portrait, DPR2 touch, physical devices,
> world/effects composition and payload/performance remain unresolved. The failed
> Chinese resize harness and normal character restoration are explicitly retained.
> [Source, actual routes, saves and limits](generated/player-qa/client-core-20260930/mobile-diary-entry/README.md).

> 2026-10-01 WebGL2 prototype109d now has primary-canvas shared Bag/Diary output,
> ordinary desktop/touch item receipts, Inspect/lifecycle/handoff evidence and
> saves136/138. Default React GL2/save140 and shared GPU/save144 remain usable.
> The complete DOM world stays below experimental UI; dark scene patches and
> increased GL2 payload remain unresolved. Original Quest entry is tiny; the
> later mobile More Diary button is recorded above. Final world/effects
> composition, whole HUD targets, DPR2 touch, physical Android/iOS and whole
> client acceptance remain open.
> [Scoped actual QA](generated/player-qa/client-core-20260930/webgl2-shared-ui/README.md).

> 2026-10-01 Web touch inventory: the shared Crystal painter has readable
> core controls and actual ordinary item/save evidence at600/640/844 landscape
> in desktop Chrome emulation. Separate entry CSS repair restores usable
> Diary/More paths. e275 now provides explicit current-document Inspect with
> complete readable paging and actual overlay input ownership, plus desktop
> hover/action and ordinary save/relogin evidence. The failed4fdd footer route
> and earlier overlap remain archived. Physical mobile, broader HUD targets,
> current DPR2 touch, WebGL2 composition and whole-client acceptance remain open.
> [Bounded evidence](generated/player-qa/client-core-20260930/touch-bag/README.md).

> 2026-09-28 Chinese newcomer/supply flow: 22 main quests, four growth claims,
> chapter/practice text, quest controls and related shop/item labels display
> Chinese without changing source IDs. Stock, price estimates, capacity and
> ordinary vendor routes are visible. Shared 1100/native 749/Node 166 tests and
> an explicit offline GPU fixture pass; three screenshots/nine cases check
> actual bounds and row separation. Legacy prose, world labels and older UI
> can still be English. Live caster 0–30/native player acceptance remain open.
> [Screenshots and limits](generated/player-qa/ui-goal-20260921/caster-supplies-chinese-20260928.md).

> 2026-09-21 multi-task guidance: one manually overridable primary task,
> full counters, nearby/other-map grouping and primary-colored map hunt areas
> are implemented using source/read-model destinations. Full UI tests 883/883;
> live visual acceptance and cross-map exit routing remain open.
> [Evidence](generated/player-qa/multi-quest-guidance-20260921.md).

> 2026-09-21 whole native UI repair pass: 2,361 drawable configured/static UI
> frames are source-pixel verified; 82 original empty slots are recorded separately.
> Five keyboard-required and seven NPC-service sprites are restored. NPC sell,
> repair and special-repair now use original drop-frame geometry and controls,
> with a complete native inventory picker, Hold and shared guarded submission.
> Full client library tests pass 876/876. Original drag/drop, reliable repair
> quoting, dynamic UI states and whole-client visual acceptance remain open.
> See [scope and remaining work](generated/player-qa/whole-ui-20260921.md).

> 2026-09-21 N5 destination guidance: pending arrival now uses a Chinese,
> flow-layout tracker card with Microsoft YaHei, readable spacing, explicit
> Bichon city (328,264), northbound direction and the exclusion of the starter
> village safe zone. A local map button opens the existing big map; a cyan
> destination square follows the imported safe-zone extent and disappears
> after completion. Three focused UI/source tests pass. This is map guidance,
> not an automatic-path or authenticated visual acceptance claim.

> 2026-09-21 unexpected window disappearance: no panic or matching Windows
> Application 1000/1001 record was found; teardown save succeeded, but the
> original process exit code was not retained. A separate real-timing regression
> reproduces stale Enter confirming a newly opened quit prompt without new
> input; this is fixed (leave tests 4/4). Native OS close now uses in-game
> confirmation (lifecycle test 1/1). Exit-source, event-loop result and launcher
> exit-code/memory diagnostics are added. The reported incident itself is not
> attributed conclusively and live stability remains open.

> 2026-09-21 profile asset repair: 139 original libraries / 60,556 drawable
> frames now cover 174 profile monsters plus BoneFamiliar and all 54 direct
> weapon/armour shapes (both genders). Source RGBA, masks, geometry and hashes
> pass. Native Monster/Gate standalone frames now use the existing strict
> metadata/path handling; atlas regressions pass 32/32. Profile and V2 valid
> source-frame closure has zero missing/unknown entries. Original blank Gate
> slots and Sheep harvested-Skeleton out-of-range behavior are separate, not
> manufactured frames. Client/Gateway release builds pass; new package awaits
> normal logout before switching. Visual and full imported-content gates are
> still open. [Repair evidence](generated/player-qa/profile-assets-repair-20260921/README.md).

> 2026-09-21 full workspace asset audit supersedes subset-closure assumptions:
> 555 monster definitions / 6,341 respawn rows examined; 164 of 174 profile
> monster definitions fail current native animation reachability. Nine of 11
> V2 targets plus BoneFamiliar fail. All 1,628 item rows' 924 Items frames and
> 214 equipment StateItem frames match original pixels, but 34 DNItems frames
> are source-empty/transparent (one invalid zero-size PNG). Current profile
> 195 item icons/ground frames pass; map-worn weapon/armour libraries exist
> for only 5 of 54 profile items. These counts describe the workspace assets
> targeted by the latest package junctions, not historical external QA packs
> or live visual acceptance. Full missing lists and limitations:
> [audit summary](generated/player-qa/all-assets-20260921/README.md).

> 2026-09-21 reported potion/missing-monster repairs: inventory was blocking
> every world click while open. Native mouse movement now uses its moved panel
> bounds, keeping drag, item-operation and modal capture. UI boundary test
> passes; native input suite passes 72/72, including an actual Walk intent
> outside the bag and no Walk inside. HookingCat image 6 was absent from both
> original PNG exports and starter atlas. All 224 original frames are restored
> in an appended atlas page, preserving the prior seven pages. Monster frame
> closure passes 8/8; native atlas test resolves all 224 frames. These are
> functional/resource results; live potion use, walking and visible HookingCat
> acceptance remain pending. [Evidence](generated/player-qa/inventory-hookingcat-20260921.md).

> 2026-09-20 Windows fixed-size policy: the native host now creates a
> 1024x768 client area with scale override 1, equal minimum/maximum size,
> resizing disabled and the maximize button disabled. Move, minimize and
> close retain their defaults. Legacy display width/height are no longer
> applied by this host while the pixel layout is fixed. The Web host is
> unchanged. Release compilation and interactive acceptance are recorded
> separately; desktop drag/DPI acceptance remains pending.

> 2026-09-20 V2 hunting-area guidance: the big map now overlays up to three
> unfinished quest targets using the nearest imported respawn area's center
> and spread, with monster name, remaining count and coordinates. Completed
> targets disappear; other-map and missing-progress targets are omitted.
> N3 guidance names RakingCat near (340,550), northeast of the starter village.
> These are possible spawn areas, not live monster positions or a new auto-path
> implementation. Two focused tests and 19 big-map regressions pass; Windows
> release package is built. Authenticated visual acceptance remains pending.
> Evidence: [hunt-map QA](generated/player-qa/numeron-hunt-map-20260920.md).

> 2026-09-20 blank Diary deployment diagnosis: after the client profile repair,
> the running Gateway was independently verified to have no MIR2_QUEST_CADENCE.
> It therefore filtered out a1's persisted V2 quests, yielding an empty list and
> 0/4 chapter progress. After user logout, the same store/EXE was restarted via
> the dedicated V2 launcher on 19900/19910. Saved level 3, N1/N2 completed and
> N3 2/4 are unchanged. Authenticated visual confirmation remains pending.

> 2026-09-20 direct-launch V2 guidance repair: the branded client was opened
> without the QA launcher's temporary guidance environment, hiding V2 Diary
> actions despite intact server progress. The Windows config now persists
> `[gameplay] quest_guidance = "newcomer-v2"` and installs both native guidance
> resources. Session-config tests pass 11/11. Saved `a1` remains Warrior level 3,
> two completed main quests and N3 at 2/4 (two Scarecrows); two RakingCats remain.
> Authenticated Diary visual verification is still pending.

> 2026-09-20 Windows map/branding follow-up: Bichon sand Back and grass Middle
> tiles shared a depth, allowing rectangular sand patches to cover grass.
> Floor depth now preserves Back/Middle/Front pass order; 39/39 map-parser
> tests pass, including the reported market cell (290,600). Dead-player V
> revival no longer also folds the minimap; the dead/alive V regression passes.
> User-requested branding is `numeron-legend of rebirth`, with the existing
> launcher gold-diamond art embedded in the EXE and applied to the native
> window/taskbar. Updated in-game visual acceptance remains open after the
> user stopped Computer Use with Escape. See
> [repair notes](generated/player-qa/numeron-branding-20260920/README.md).

> 2026-09-20 Windows fast-run self-label candidate: a player capture at
> `(232,607)` shows the body roughly one run step ahead of its `a1` name and
> self HP bar. The native renderer can accept a corrected/predicted self tile
> while the raw gameplay snapshot remains unchanged; the overlay's early
> return previously ignored that tile change. The overlay now invalidates on
> the renderer payload's self object/tile or scene-center change. A no-new-raw-
> snapshot two-cell run regression and the full 18/18 entity-overlay suite
> pass. The Release EXE at `C:/mir2-fast-run-anchor-20260920/native-client`
> (SHA-256 `75F94CED5B728B30D2F5C65849F496A5F7DC9C84ABA3A2BF009C66E836D83834`)
> started and connected to the local V2 Gateway; the visible login screen was
> captured. Authenticated sustained-running capture and original Crystal
> comparison remain open; `visualAccepted=false`.

> 2026-09-20 live follow-up after reboot: the rebuilt EXE connected to the
> isolated V2 Gateway at `127.0.0.1:19910`; the user manually logged in as
> `a1`. With AutoRun enabled, sampled outdoor frames near `(230,609)` and
> `(342,496)` kept the body, `a1` name, and self HP bar on the same screen
> anchor. The character died near the eastern shore, then revived in town via
> the normal `V` action; AutoRun was turned off. The interval between samples
> included operator analysis time, so the coordinate delta is **not** a
> measured speed result. A subsequent run reached `(315,642)` behind a castle
> roof, where the character sprite is occluded while the name/HP remain
> visible. Normal movement back to `(309,636)` produced a readable stopped
> frame with the self sprite, name, and HP bar horizontally aligned and full
> 30/30 HP; AutoRun remains off. Continuous right-button-hold timing and
> original-client comparison remain open; `visualAccepted=false`.

> 2026-09-20 Windows ground-drop visibility candidate: a live player screenshot
> showed `WoodenSword` text over a Bichon roof with no discernible item sprite.
> The authoritative drop has image 30 and the original `DNItems/30.png` is
> present (36x25). The native world pass now draws an item after its own cell's
> front map image, matching Crystal's row order. When DropView is on, its
> post-world name layer also displays that same DNItems frame at the exact
> ground-item position, so later-row roofs cannot leave only a floating name.
> Missing/empty frames are skipped, and pickup state is unchanged. The focused
> item test and 17/17 entity-overlay tests pass; the Release EXE was built and
> opened at the login screen from `C:/mir2-ground-drop-20260920/native-client`
> (SHA-256 `349EB41DCF998325753D348C47DC87B62A940106736646F139BA471EDB4683F7`).
> An authenticated in-world capture, roof/drop pickup check, and Crystal
> same-scene comparison are still pending. `visualAccepted=false`.

> 2026-09-14 Windows-native R15 minimap/attack-facing repair: the Bichon
> minimap source and the `(252,520)` crop were both non-black, locating the
> reported dark-scene failure in UI composition rather than asset export or
> coordinate mapping. The 120x108 minimap content is now rendered immediately
> above the HUD skin and below chat/dialog layers, so the HUD's transparent map
> opening remains readable while the retained darkness/light pass is active.
> Four focused native-UI minimap tests and the 87-test gameplay-bridge suite
> pass; the optimized Release build is packaged as
> `mir2-platform-windows-facing-minimap-r15.exe` (SHA-256
> `2964731EDE4C06698694A02BCDD3DEB5EF7695F31AEA0BCA477218A54348EC07`).
> A 1024x768 GPU capture at Bichon `(285,585)` with darkness active
> (`setting=1`, `mapDarkLight=0`) shows the terrain and entity markers inside
> the minimap; image SHA-256 is
> `7fe91c637c2edf3d19ae5cda8b41f006e56333181bbfcb2e26687ec3838e4383`.
> The same running build acquired the adjacent Deer target and faced the actor
> toward it without moving the authoritative camera centre. This closes these
> two reported rendering leaves; package-wide Crystal parity and final human
> acceptance remain open. `formalCandidate=false`, `accepted=false`,
> `visualAccepted=false`.

> 2026-09-11 natural-journey J17 Ground Harvest checkpoint: installed r9
> Native SHA
> `B9E01520D6AFED67546EA60E897395FA95917CDBDCA85430E70DAC5211C477BA`
> passes the final harvest set 8/8, Crystal Alt-click 2/2, Alt+NPC 1/1 and
> same/empty-tile Alt 1/1. Its first launcher attempt referenced r8 metadata;
> the hash guard rejected it before process start, and the corrected r9
> reference launched successfully. Live relog restored Warrior level 5, EXP
> 38%, HP 44/44, gold 230 at (289,584), closing only the level-5 persistence
> checkpoint. Same-tile/empty-ground Harvest remains under live review. Far
> angles still use tile approximation and 2500ms is a conservative retry bound,
> so full input/timing parity remains open. `formalCandidate=false`,
> `accepted=false`, `visualAccepted=false`; evidence:
> NATURAL-THREE-CLASS-JOURNEY-20260911.md.

> 2026-09-11 natural-journey J11 sprite/input checkpoint: q6 HookingCat appears
> as a name without a monster sprite, blocking ordinary play. The isolated
> Monster/006 replacement contains 224 unique atlas rects with zero missing or
> out-of-bounds frames and the expected source Lib hash, but it is not installed
> or visually accepted. A Deer corpse highlighted blue under r5, yet three real
> harvest attempts still yielded no reward. The default-off native input trace
> work remains in progress. Computer Use stopped when the user pressed Esc and
> no further UI interaction occurred. `formalCandidate=false`, `accepted=false`,
> `visualAccepted=false`; evidence:
> NATURAL-THREE-CLASS-JOURNEY-20260911.md.

> 2026-09-11 natural-journey J09 UI checkpoint: the `groundDrops`-driven Recent
> Ground Pickups panel is identified as a QA helper and hidden from the ordinary
> Crystal player HUD. Two focused tests and the optimized-code `dev` Native
> build pass; SHA
> `E5FFCEB42AE0DB28E26E883F755B373B3A55A0EC35DB6C717342B1981DC09BD8`
> is not installed and has no live acceptance yet. q5 completion exposed a
> separate stale Back-page issue: q6 Accept worked after fully reopening the
> NPC, while the source fix awaits build/retest. Six equipped items persist,
> but CopperRing may be missing from the character page and remains under
> investigation. `formalCandidate=false`, `accepted=false`,
> `visualAccepted=false`; evidence: NATURAL-THREE-CLASS-JOURNEY-20260911.md.

> 2026-09-11 NPC-dialog partial visual checkpoint: Board text no longer leaks
> raw markup; Create Hero labels correctly, down-page reveals ReviveHero,
> SealHero and Use, Use opens the server shop list, and its last page exposes
> Hairdresser/Close. The separate QUEST control opens five tasks; top-right X
> closes both dialog and task list. The feature-enabled native pagination test
> passes 1/1 with 805 filtered; the related native UI selection passes 4/4 and
> includes that same case. The native build passes. This is a usability
> repair within 440x224, not full Crystal layout parity: inline coloured links,
> large buttons and duplicate fallback footer entries such as Use/Weapon shop
> remain. Hero business actions were not live-tested. `formalCandidate=false`,
> `accepted=false`, `visualAccepted=false`. Evidence:
> generated/player-qa/quest-progression-20260911/README.md.

> 2026-09-11 combat-fix partial visual checkpoint: V revived a persisted HP=0
> character to Bichon (288,616), showing 224/224. A position-only continuation
> at map 1 (278,180), with quest counts unseeded, recorded one natural
> ForestYeti kill (0/8→1/8; EXP 14.29%→14.31%). On a respawned Yeti, the HUD
> showed 137/224→106→63→27→0 instead of full health followed by immediate death.
> Relog returned to Bichon (288,616) at 224/224 with Yeti 1/8 and 14.31%
> retained; the persisted experience value is 20036.
> Gateway owner-health ID and native dead-V regressions pass 1/1 each; both builds
> pass. Trees and night lighting obscured parts of combat and attack operation
> remained awkward, so animation is not visually accepted. Eight Yeti kills,
> natural Oma, full route/balance and three-class coverage remain open.
> `formalCandidate=false`, `accepted=false`, `visualAccepted=false`. Evidence:
> generated/player-qa/quest-progression-20260911/README.md.

> 2026-09-11 partial native quest visual checkpoint: the Board UI verified the
> two-choice cap and abandon/replace path. After live-found fixes, the objective
> renders `Defeat OmaFighter (0 / 10)` without duplicate progress and the
> abandoned Skeleton row remains hidden once Forest Yeti fills the second slot.
> Level-15/20 milestone gold persisted across relog with no duplicate entry.
> An explicitly `QA-progress-seeded` continuation verified native hand-in and
> bonus 0/2→1/2→2/2/claim/relog, but natural kills remain unverified. Focused
> newcomer tests pass 8/8. `base-dress-equipped-fixed.png` also records the
> separate BaseDress slot fix working in the running native package. This is
> partial evidence; `formalCandidate=false`, `accepted=false`, and
> `visualAccepted=false`. Details and PNGs:
> generated/player-qa/quest-progression-20260911/README.md.

> 2026-09-11 internal QA package ready: fresh optimized-dev client/Gateway, 50,400
> asset hashes verified. Isolated level-20 Board fixture and protocol smoke pass
> (10 definitions, 6 eligible quests). Desktop takeover awaits user pause while
> DeltaForce runs. No native visual acceptance or signed Release claim. Evidence:
> generated/player-qa/quest-progression-20260911/README.md.


> 2026-09-11 newcomer progression follow-up implemented: 1–40 guidance (138
> quests per class / 144 source IDs), three daily choices with a two-quest cap,
> current-day 2/2 claimable bonus, and one-time gold milestones at 15/20/25/30/35/40.
> Ten independent IDs are server newcomer-v1 only; default Crystal is preserved.
> Exact Board binding, distinct claims, persistence/profile isolation and daily
> high-watermark checks pass. Gateway now sends changed quest snapshots even on
> Tick/KeepAlive without expanding ordinary movement snapshots. Verification:
> Node193, Bevy88, Windows1, simulation68, store6, Gateway29 pass. No package or
> live visual/three-class balance acceptance. This is optional content, not a
> new Crystal parity percentage. Next: matched package and live Board/Diary/
> reward/relogin acceptance. Details: QUEST-PROGRESSION-CADENCE.md.

> 2026-09-11 newcomer follow-up: 1–25 guidance (110 distinct quest IDs), server
> Daily/Weekly/Repeatable groups preserved over static optional categories.
> Native88 and Node193 pass; Gateway28 verifies repeatable Finish ACK compatibility.
> New package/screenshots/normal progression remain open. Optional guidance is
> product UX, not an increase in accepted Crystal parity. QUEST-PROGRESSION-CADENCE.md.

> 2026-09-11 user-authorized optional quest UX: startup newcomer-v1 profile
> groups the native diary, orders NPC tasks ready-first, and adds scrollable
> guidance for the first 15 levels. Fixed zero item rewards are hidden in this
> mode, selectable rewards retain protocol indices and display zero explicitly.
> Crystal default remains unchanged. Rust quest 87 / Node 192 pass; current
> Windows package and visual acceptance remain open. This intentional optional
> UX is not counted as Crystal 1:1 acceptance. See QUEST-NEWCOMER-ACCEPTANCE.md.

> 2026-09-10 native Keyboard menu: Crystal Title119 editor is connected to
> actual key capture, strict/relaxed modifiers, Delete unbind, full reset,
> grouped scrolling, movable window, and application-scoped atomic JSON save/load.
> The 96 source entries remain editable; 49 currently have native handlers and
> 47 remain pending system integration. Existing hard-coded conflicting menu,
> belt, skill, quest and screenshot keys now read the saved catalog. QA capture
> is Ctrl+Shift+F12; ordinary PrintScreen release is configurable and F12 opens
> Options. This is `implemented_pending_functional_and_visual`, not accepted.
> Exact handler matrix and remaining input/layout differences:
> `docs/generated/player-qa/native-keyboard-20260910/README.md`.


> 2026-09-10 menu parity is in progress. Shared Ranking presence now uses
> stable account/character identity across local factory Zones, excluding
> replicas and leaving players (simulation 3/3, Gateway integration 1/1).
> Creature updates now preserve server-owned properties, reject fabricated
> pets and separate summon state from pickup mode; authority/migration 5/5,
> simulation regressions 4/4 and Gateway regressions 8/8 pass. Legitimate
> creature acquisition/actor rendering, distributed ranking and final native
> visual acceptance remain open. See `NATIVE-MENU-CRYSTAL-PARITY.md`.
> No global completion percentage or final acceptance is claimed.

> 2026-09-09 trade merge/reorder: native item selection now sends exact-UID
> merges across Inventory/Trade and slot moves/swaps within own Trade.
> Partial merges preserve the remainder and target identity; server validates
> metadata, capacity, logical grid membership and editable offer state.
> Shared routing blocks edits when either participant is prepared and refreshes
> the partner offer. Trade merges require exact ACK/NACK, not inventory-delta
> inference. Evidence: `docs/generated/player-qa/native-trade-merge-20260909/README.md`.
> This deliberately completes a client interaction rejected by this Crystal
> server checkout. Full custody/prepared editing and live package/visual gates
> remain open; no global UI completion or acceptance percentage is claimed.

> 2026-09-09 native trade item operations: inventory-to-own-trade deposit and
> own-trade-to-selected-bag retrieval are connected to ordinary packet intents.
> Server retrieval now validates the exact offered instance and destination,
> preserves item identity/metadata, and rejects occupied/out-of-capacity cells.
> UI selection is bound to item state and the current exchange; guest cells,
> locked trades and covering dialogs reject input. Failed host sends release
> only the matching unsent operation. Automated evidence and remaining gaps:
> `docs/generated/player-qa/native-trade-items-20260909/README.md`.
> Merge, trade-slot movement, complete item custody and live Windows visual
> acceptance remain open. All 33 global UI backlog IDs remain open;
> `accepted=false`, `visualAccepted=false`, `globalParityPercent=null`.

> 2026-09-08 repeated StartGame: bootstrap completion was incorrectly scoped to socket lifetime. Successful StartGame now rearms initialization; next accepted snapshot completes world entry once.
> Evidence: `docs/generated/player-qa/windows-reenter-world-20260908/README.md`. Live verification remains pending.

> 2026-09-08 self/camera mismatch: cached owner attack coordinates were applied after UserLocation, overwriting the sprite position but not scene center. Owner transform now applies after object overlays.
> Evidence: `docs/generated/player-qa/windows-self-camera-20260908/README.md`. Live acceptance remains pending.

> 2026-09-08 user still reports rubber-banding after the single-in-flight build. Native input now aligns wire/prediction mode, uses real-time deadlines and send-based run eligibility, and prevents stale snapshots from overriding packet position.
> Evidence: `docs/generated/player-qa/windows-run-clock-20260908/README.md`. Long-hold live acceptance remains open.

> 2026-09-08 native movement backpressure: one unacknowledged move; delayed ACK no longer triggers speculative queuing or timeout ownership loss/retry.
> Evidence: `docs/generated/player-qa/windows-movement-backpressure-20260908/README.md`. Live running-under-load acceptance remains open.

> 2026-09-08: native monotonic startup/atlas/connect/login/StartGame timing added.
> CPU/transport milestones explicitly do not assert first rendered frame or playable readiness.
> Evidence: `docs/generated/player-qa/windows-timing-20260908/README.md`.

> 2026-09-08 native owner swing: Zone ObjectAttack used a different actor ID
> from the personal SelfPlayer snapshot. Owner-facing execute returns now
> normalize own ObjectAttack/ObjectRangeAttack IDs after shared-state and
> observer processing. Two-session public regression passes (owner local ID,
> observer/shared-state Zone ID). Full Gateway regression: 696 passed, zero
> failed, one PostgreSQL-environment ignore. Isolated localhost service updated;
> live swing acceptance awaits manual login.
> Evidence: `docs/generated/player-qa/windows-owner-swing-20260908/README.md`.
> This does not close asynchronous combat identity or global visual parity.

> 2026-09-08 ground-item/name fix: native world rendering now consumes
> groundDrops using original DNItems (5280 exported frames), including gold
> quantity frames. Names use intrinsic-text centering; drop-name camera motion
> is applied once. Windows 554 tests and package/verifier self-tests pass.
> Evidence: `docs/generated/player-qa/windows-ground-label-20260908/README.md`.
> New development output still needs live visual acceptance; global gates false.

> 2026-09-08 resumed Windows gameplay QA: single-click chase, repeated damage,
> post-hit red monster bars, death-time bar removal, and a Deer/Venison drop
> were observed in the running combat-animation development output. Evidence:
> `docs/generated/player-qa/windows-combat-animation-20260908/live-*.jpg`.
> Attack/Struck animation continuity is still visually unaccepted; these
> sampled stills do not establish animation parity. Global gates stay false.

> 2026-09-08 combat animation/health feedback: preserve newer self ObjectAttack
> hints over retained movement, admit every packet projection to ActionFeed,
> and exclude combat actions from movement-echo filtering. Actual renderer-frame
> regressions verify self/monster Attack1 then Struck. Monster bars now render
> original Prguse2/0+1 with packet health and expiry/generation. Ordinary Zone
> hits use Expire=0, so a new confirmed normal/critical positive damage event
> supplies a five-second local display window; snapshots/healing do not renew.
> Nodes remain retained, expiry/removal works during render-center delays.
> Windows 553/553 and both package/verifier self-tests pass. Development output:
> `C:\mir2-combat-animation-20260908`; signed Candidate unchanged. In-game
> visual acceptance and broader combat parity remain open. Evidence:
> `docs/generated/player-qa/windows-combat-animation-20260908/README.md`.

> 2026-09-08 Windows hover/combat input: NPC/monster highlights now use Crystal's
> additive 0.3 redraw. Ordinary monster clicks retain a target, walk to a
> reachable attack neighbour, follow movement and repeat attacks after arrival
> ACK using the source level/attack-speed interval. Death/removal, manual input,
> blocked UI, focus/session/identity changes cancel pursuit. Archer class-weapon
> range behaviour remains stationary. Windows 546/546 pass; independent review
> is closed. Authenticated in-game visual/combat acceptance and broader combat
> parity remain open. Development output: `C:\mir2-combat-20260908`; signed
> Candidate unchanged. Evidence:
> `docs/generated/player-qa/windows-combat-input-20260908/README.md`.

> 2026-09-08 NPC quest-marker flicker: markers now retain an independent image
> entity keyed by objectId while names/damage overlays rebuild. Both original
> animation frames keep strong asset handles across swaps and NPC visibility.
> Original 500 ms animation and camera offsets remain. A 90-frame ECS regression
> checks stable entity/position, frame swaps, quest changes, movement, removal
> and disconnect. Windows 541/541 pass; independent review found no blockers.
> Human in-game flicker acceptance remains pending; signed Candidate unchanged.
> Evidence: `docs/generated/player-qa/windows-npc-marker-20260908/README.md`.

> 2026-09-08 login-door transition: accepted LoginSuccess now hides the login
> dialog, plays Crystal ChrSel frames 1..18 at 100 ms per frame, and only then
> reveals character selection. Frame 0 remains the idle door; all 19 images
> must be resident before advancing. The original Sound/100.wav plays once,
> independently from button sounds. StartGame and developer auto-start wait
> for completion; disconnect cancels stale selection. UI 603/603 and Windows
> 540/540 pass. Development hotfix: `C:\mir2-login-door-20260908`; actual startup
> reached the connected login window. Authenticated visual/audio acceptance
> remains open, as do other login/selection UX gaps. Signed Candidate unchanged.
> Evidence: `docs/generated/player-qa/windows-login-door-20260908/README.md`.

> 2026-09-08 sustained right-button running: accepted world presses now retain
> a Run hold and refresh the cursor target until mouse-up, so reaching the first
> clicked tile does not stop a held run. Outside-world presses remain inert.
> Input 44/44 and Windows host 539/539 pass; an optimized development hotfix is
> available at `C:\mir2-right-hold-20260908`. Manual held-mouse acceptance is
> pending; signed Candidate unchanged. Evidence:
> `docs/generated/player-qa/windows-right-hold-20260908/README.md`.

> 2026-09-08 Windows repackaging checkpoint: clean snapshot `4264b9149`
> builds and passes the signed Candidate verifier (37,534 files). Sound104
> package/allowlist mismatch and malformed-Unicode PE scan false positive are
> fixed with self-tests and independent review; 704 required paths are checked
> against the verifier allowlist. The exact final EXE opened the real native
> login window and connected to the rebuilt isolated local Gateway. Authentication
> is a manual handoff; in-game trade/map, DPI/soak/human acceptance remain open.
> Evidence: `docs/generated/player-qa/windows-repackage-20260908/README.md`.
> `accepted=false`, `visualAccepted=false`, `globalParityPercent=null`.

> 2026-09-08 Windows trade gold custody checkpoint: positive incremental offers
> now debit the wallet immediately; preparation/recovery only debit outstanding
> gold. Persisted heldGold preserves legacy snapshots and is independent from
> prepared item custody. Cancel/teardown and orphan positive-hold recovery refund
> once; save failure restores custody. Cap/materialization failures retain final
> retry authority. Ledger bootstrap occurs before the first eligible debit.
> Any prepared participant blocks gold and item edits; item failure ACKs prevent
> withdrawing an offer while reusing the peer's previous confirmation.
> Simulation 1491 unit + 374 unique integration tests are verified. Gateway resolved
> coverage is 695 passed / one existing environmental ignore: the initial full
> run had one queued-notification fixture assertion, corrected by a test-only
> change and a passing 10/10 gold rerun. Production code did not change for that
> correction. Format/diff and independent bounded review pass. Exact raw results:
> `docs/generated/player-qa/native-ui-parity-20260908-trade-gold/README.md`.
> Next: separate confirmation tickets from exact held-item custody, then editable
> prepared offers and native deposit/retrieve/merge. Prepared unlock still cancels/
> refunds; source capacity rejection retention, zero-held orphan cleanup, request
> throttle/error chats, screenshots and all 33 backlog IDs remain open. No UI/
> Windows-host rerun, package, interactive launch, live-store write or deployment
> occurred. `visualAccepted=false`, `accepted=false`, `globalParityPercent=null`.

> 2026-09-08 Windows trade invitation/private-pair checkpoint: native source
> MirMessageBox Yes/No and cancellation OK, invitation revision ownership,
> keyboard disposal and modal input isolation are implemented. Shared Gateway
> invitations go only to the facing recipient; accepted reciprocal presence
> pairs own guest gold/item notifications and settlement matching. Refusal,
> teardown, old-cleanup/new-invite ordering and bootstrap failure are covered.
> Native UI 598/598, Windows 537/537, Gateway 685 passed / one existing ignored,
> and new Gateway security tests 13/13 pass;
> the final full Gateway result and source hashes are recorded in
> `docs/generated/player-qa/native-ui-parity-20260908-trade-invitation/README.md`.
> Next: positive-delta/immediate editable gold escrow and bilateral unlock,
> then exact item custody and native deposit/retrieve/merge operations. Cells
> remain read-only; prepared unlock still cancels/refunds. Request throttle,
> complete error chats, original paired screenshots, package/light/DPI/soak/
> legal/signing/human gates and all 33 IDs remain open. No interactive launch,
> screenshot, production rollout or live-store write occurred this round.
> `visualAccepted=false`, `accepted=false`, `globalParityPercent=null`.


Last updated: 2026-09-03

> 2026-09-03 native launch regression: a relocated executable's resource
> junction lost the separate development map-layout path, producing actors
> and HUD over a black world. The map locator now resolves the real directory,
> and startup checks/decode map 0 before opening. The screenshot coordinate
> produces 849 map draws with all 221 local image references present; native
> host tests pass 537/537 serially. Default-parallel GameShop queue failures
> remain separately recorded. The repaired EXE is open at login after explicit
> Computer Use resumption; authenticated world visuals still need manual login.
> No full-pack download, resource migration or store reset was performed.
> Evidence: `docs/generated/player-qa/native-ui-parity-20260903-map-relocation/README.md`.
> This is not closure of any whole-window/global parity or human acceptance gate.

> 2026-09-03 trade completion source/headless checkpoint: WN-ITEM-002/003
> no longer receive a premature completion when a shared offer is merely
> prepared. Actual delivery completes once; the durable path waits for its
> saved projection. Simulation 1491/1491 + dedicated 7/7 and Gateway 672 passed / one existing ignored
> pass, as do native UI 591/591, Windows 534/534, runtime 212/212 and UI core
> 43/43. Native source/assets are unchanged. Exact evidence and diagnostics:
> `docs/generated/player-qa/native-ui-parity-20260903-trade-completion/README.md`.
> Source gold delta/immediate escrow, invitation/pair ownership, keep-offer
> capacity rejection, unlock/re-edit and exact item operations remain open;
> both native trade grids are still read-only. This is not full trade or
> packet-order parity. No GUI/capture while Computer Use remains paused.
> All 33 IDs and original-pair/package/light/DPI/soak/legal/signing/human gates
> remain. visualAccepted=false, accepted=false, globalParityPercent=null.

> Historical 2026-09-03 accepted-exchange TradeDialog source/headless checkpoint:
> WN-ITEM-002/003 now include the original independent own/guest window
> geometry/art, ten fixed sparse cells per side, current-count full-bitmap
> icons/hints, independent bag, basic dragging and source amount/lock/close
> controls. Exact partner/nonce/slot/UniqueID read-only projection and explicit
> exchange/unlock revisions prevent compacted cells or stale modal ownership.
> Escape does not cancel trade, as in Crystal Closeall. Native UI 591/591,
> Windows 534/534, runtime 212/212, UI core 43/43, the 11-test/924-image gate
> and 561 original PNG checks pass; all 552 prior Prguse/Title PNGs are intact.
>
> This is not complete trade: the grids remain read-only and legacy bag/to=0
> actions were removed. Original invitation/cancel MirMessageBoxes, Gold
> 106.wav, complete amount editing/GDI, item operations/overlays and full
> window input/topmost remain open. Candidate also sends S.TradeConfirm before
> mutual/durable settlement and has non-source TradeGold overwrite/deferred-
> debit semantics. Source-correct client completion handling exposes this
> backend gap; it must not be disguised as a partner lock. Next work must fix
> transaction phases and conservation before claiming full UI interaction.
> Backend source is unchanged here. Report:
> `docs/generated/player-qa/native-ui-parity-20260903-trade-dialog/README.md`.
> Computer Use stays paused after user Escape; no new GUI capture exists.
> All 33 IDs and original-pair/package/light/DPI/soak/legal/signing/human gates
> remain; `visualAccepted=false`, `accepted=false`, `globalParityPercent=null`.

> 2026-09-03 native primary-item true-size checkpoint: WN-ITEM-001/002/003,
> WN-CHAR-001 and the belt leaf of WN-HUD-001 now have source alpha-size
> centring/full-bitmap, valid source Items/0 and actual-PNG regressions.
> Equipment's fourteen cells and personal storage now use original 36x32
> bounds; bag/grid/NPC-row clipping is removed, source StackSize shows count
> 1 and persistent belt loading/clearing cannot leave a stale/white icon.
> Warehouse-side, trade and amount icon regions also use the helper, while
> their full windows, operations and overlays remain separate open work.
> All 1003 exported PNGs match original RGBA/metadata, 5015 node geometries
> pass, and full native UI 562/562, Windows 528/528, runtime 212/212, UI core
> 43/43, the 11-test/924-image gate and formatting are green. The original
> fixture reproduces exactly after newline normalization.
>
> This supersedes the primary centring/zero-image gaps in the historical
> Guild note below. No new GUI capture or final-source same-EXE claim is
> made while Computer Use remains paused after user Escape. Full trade/
> GameShop/Quest/other layouts, source item operations/state overlays,
> FloorItems, WN-CHAR-002 slot contracts and all 33 backlog IDs remain open
> as recorded, with original-pair/package/light/DPI/soak/human gates intact.
> `docs/generated/player-qa/native-ui-parity-20260903-item-true-size/README.md`.
> `visualAccepted=false`, `accepted=false`, `globalParityPercent=null`.

> Historical 2026-09-03 native Guild storage checkpoint: WN-ITEM-002/003 now include
> source-shaped eight-column storage, fourteen authoritative rows and an
> eight-row viewport with original integer scrolling and stable slot IDs.
> Current counts select source UserItem.Image; alpha-bound GetTrueSize centres
> the unchanged full PNG, including legitimate Items/0. Original tab/scroll
> assets, leader-only withdrawal and MirAmountBox replace generic buttons and
> the inline amount field. Amount ownership, stale guild/rank/balance, zero,
> pending/cooldown and focus/session cases have headless coverage; no client
> gold mutation is introduced. Final native UI 551/551, focused Guild 44/44,
> Windows 527/527, runtime 212/212, stack integration 4/4,
> UI core 43/43, protocol 40/40, game-data 39/39, the 11-test/924-image gate
> and 41 direct original-frame RGBA/geometry checks pass.
> The source alpha audit finds 550/1003 frame-size differences and 478 altered
> 35-pixel-cell offsets. Primary bag/belt/equipment/storage/NPC paths still use
> PNG-frame centring and need correction; earlier PNG pixel checks did not
> compare against original GetTrueSize. Guild/coin is the only corrected path
> in this round. Primary Items/0 handling also remains open.
>
> This is not fresh GUI evidence: user Escape still pauses Computer Use, and
> no new application EXE or capture was produced. Guild item dragging/store/
> retrieve/merge, source state overlays, full WinForms text editing/GDI raster,
> Guild window movement/topmost pointer dispatch and cross-action LastGuildMsg
> remain open. Current Windows/runtime/integration checks and exact scope:
> `docs/generated/player-qa/native-ui-parity-20260903-guild-storage/README.md`.
> Trade/other surfaces, FloorItems, prior original-pair/package/light/DPI/human
> gates and all 33 backlog IDs are retained; `visualAccepted=false`,
> `accepted=false`, `globalParityPercent=null`.

> 2026-09-03 native source stack-image checkpoint: bag/belt/equipment/storage
> and NPC goods now select the original UserItem.Image from exact Info and
> live count before resolving true-size geometry. Ordinary known source
> identities override stale cached icon values; GameShop/Quest/craft-shadow/
> mail-list base-image exceptions stay intact. Windows 527/527, game-data
> 39/39, stack integration 4/4 and the 11-test/924-image asset gate pass.
> Full Simulation 1491/1491 and Gateway 667 passed / one existing ignored
> subsequently passed on the same final source; no failed test was excluded.
>
> A read-only check of the initial inventory PNG matches all 24 fixed bag
> cells (6,161 opaque RGB samples), with 86 rejected wrong frames and 96
> rejected one-pixel translations. It excludes count-label pixels, alpha
> edges and other panels. Crucially, both captured binaries predate the
> final ordinary-item base-icon correction, and the user stopped Computer
> Use before manual transitions. No current-source EXE or interactive
> stack acceptance is claimed, and foreground work must remain paused until
> the user explicitly resumes it. Report and unchanged draft sidecar:
> `docs/generated/player-qa/native-ui-parity-20260903-item-stack-images/README.md`.
>
> WN-ITEM-002 remains open for guild/trade and every other actual-item surface,
> ground FloorItems, durability/locked/selected/sealed/unavailable overlays,
> final-source manual input, original paired state, real DPI and human QA.
> WN-CHAR-002 source operations/raw-versus-normalized belt addressing and all
> other backlog IDs remain unchanged. No prior denominator is discarded;
> `visualAccepted=false`, `accepted=false`, `globalParityPercent=null`.

> 2026-09-03 Windows-native full-catalogue icon/high-armour checkpoint: the
> original export now follows all 1,628 catalogue rows (913 base Images) plus
> the eleven Amulet/Poison stack images from `Shared/Data/ItemData.cs:641-681`.
> A mandatory 924-image gate checks catalogue/source identity, metadata,
> geometry, decoded pixel and PNG hashes, including missing file-plus-meta,
> substitutions, new catalogue images and missing quantity variants. The
> source exporter added 643 PNGs, preserving all 360 prior files byte-for-byte.
> Exact CArmour/09 and /10 exports add all 3,232 source frames. Direct source
> RGBA/geometry checks and 512 complete Warrior gender/direction/standing/
> walking/running composites pass through the existing actor-library path.
>
> Debug EXE SHA-256
> `55CA1D61A6977F164B1D6222DE2AAAFD4E21FEF4A1C5DB898EBEE6FF0FBDB993`
> produced four actual HeavenArmour/MirArmour gender captures and a same-process
> auto-Character -> manual Inventory -> unequip -> Items/595 in first bag cell
> -> re-equip cycle. The completed auto-capture helper now returns before
> target preparation; its App regression also preserves page/tab/later-notice
> state. The live fixture does not unlock ITEMS II, so its manual screenshot
> proves panel release on page 0, not live second-page retention. Evidence:
> `docs/generated/player-qa/native-ui-parity-20260903-item-actor-assets/README.md`.
>
> Windows 524/524, item suite 11/11 plus 924-image closure, original exporter
> regressions and the ten-control Windows PowerShell 5.1 self-test pass.
> Full frontend logic still fails on the absent local WebGPU WASM file;
> all five later checks pass independently. This supersedes the specific
> missing MirArmour icon/head-only self-body/one-shot-helper notes below.
> Runtime quantity-image selection and live stack tests, world CHumEffect
> wings, name-only remote-player observations on rapid QA reconnect, complete
> class/action/gear and specialized-item surfaces, original same-state pairs,
> trusted package/light, real DPI and human acceptance remain open. Eight
> sidecars stay draft/ineligible; the later additive eleven-frame export has
> a separately recorded manifest identity. No denominator leaf was removed.
> `visualAccepted=false`, `accepted=false`, `globalParityPercent=null`.

> 2026-09-03 Windows-native Character wing/unequip checkpoint: Simulation now
> derives `WingEffect` from the wearer's exact, non-broken real armour template,
> and Gateway preserves self-only authority, explicit zero clearing and partial
> snapshot/session-reset semantics. CharacterDialog draws only source effects
> 1/2, only while armour exists, using original `Prguse2/1202..1205` intrinsic
> rectangles and SourceAlpha + One DrawBlend before the existing StateItem
> armour -> weapon -> helmet-or-hair layers. Four retained material handles
> avoid per-frame GPU asset churn; startup requires all four source PNGs.
>
> Final Debug EXE SHA-256
> `8BA170AA654FCE1EB911033203C22F018EAD2A4B973F04A4F80A8679E2FF6F20`
> produced four gender/effect captures and a real-pointer same-process
> WingOneM cycle at BichonProvince `(290,620)`: wing present -> armour removed
> and wing absent -> exact item in bag cell 0 -> re-equipped and wing restored.
> This exposed and fixes the old `RemoveItem { grid: equipment, to: -1 }` and
> the raw-array/normalized-bag offset mismatch. The current Gateway takes a
> normalized 0-based bag destination; it must not receive Crystal's raw +6.
> The client waits for authoritative state and refuses stale/full-bag targets.
>
> Native UI 519/519, runtime 212/212, Windows 521/521, Simulation 1491/1491,
> Gateway 666 active/1 ignored, wing integration 5/5 and the affected recall
> integration 2/2 pass. The formerly failing Archer atlas test now owns its
> deterministic manifest availability and tests the real unavailable-library
> fallback; production routing is unchanged. Evidence and source/hash ledger:
> `docs/generated/player-qa/native-ui-parity-20260903-character-wings/README.md`.
> This supersedes older notes below that wing data is absent or the Windows
> suite has a known Archer failure. The generic item-operation popup,
> name-based equip destinations, belt-first amulet/merge path, full Character
> tabs/class/gear matrix, missing MirArmour icons `Items/595` and `605`,
> head-only high-armour world actors, original same-state pairing, trusted
> package/light provenance, DPI and human acceptance remain open.
> `visualAccepted=false`, `accepted=false`, `globalParityPercent=null`.

> 2026-09-03 Windows-native Crystal NPC goods-cell checkpoint: Candidate now
> renders each NPC catalog entry as Crystal's source `MirGoodsCell` rather
> than the former combined text row. The full `205x32` cell is the click/hover
> target; original item pixels are centred in the `40x32` icon region; name,
> yellow count and localized-shape `Price: N gold` text use `(44,0)`,
> `(23,17)` and `(44,14)`; selection uses the Lime outline plus x=40 divider;
> and the original `Prguse/550` marker follows `!IsShopItem ||
> MultipleAvailable`. `NPCGoods.HideAddedStats` is retained end-to-end and
> passed only to this surface's shared Crystal hint renderer, which suppresses
> added attack/defence values and `Cursed` without suppressing base or other
> bind text. These rules come from `MirGoodsCell.cs:20-140`,
> `NPCDialogs.cs:1071-1082,1348`, `ServerPackets.cs:3082-3104` and
> `GameScene.cs:4199`.
>
> The exact freshly built client EXE SHA-256 is
> `159B13E722451C6F44B036C6B3ABD141E19362EDB28ED29180F34C6849A7DD8A`.
> Real Windows pointer input followed login -> Scott -> View at
> `BichonProvince (288,616)`. Run `npc-shop-20260903-r2` records the populated
> baseline and selected/hover states at
> `docs/generated/player-qa/native-ui-parity-20260903-npc-shop/`; both F12
> sidecars report `panel=NpcShop`, 1024x768 and DPI 1.0. Native UI passes
> 514/514, runtime 212/212, and Windows is 519/520 with only the existing
> Archer atlas fixture failure. The source-sized NPC row leaf is bounded, but
> duplicate/sub-goods topology, exact layouts/live captures for GameShop,
> quest rewards, guild storage and trade, trusted package/light provenance,
> DPI coverage and human comparison remain open. `visualAccepted=false`,
> `accepted=false`, `globalParityPercent=null`.
>
> 2026-09-03 Windows-native remaining item-tooltip surfaces: NPC shop,
> GameShop, fixed/selectable quest rewards, guild storage and trade now carry
> the complete Crystal tooltip source into the shared hint renderer. The route
> distinguishes actual wire `UserItem` instances from Crystal's synthetic
> catalogue previews, preserves GameShop count/full durability, keeps Quest
> reward count outside the synthetic item, applies viewer-specific
> `GetRealItem`, and fails closed on missing or duplicate indexes. Rich hover is
> active even for non-clickable reward/shop cells and cleans up through the
> same delayed lifecycle as personal items. Native UI passes 511/511; Windows
> is 519/520 with only the existing Archer atlas fixture assertion, and the
> current Windows Debug build succeeds. This removes the sparse-model blocker
> recorded in the older checkpoint below, not the remaining panel-layout work:
> exact GameShop/guild/trade geometry, source-sized hit regions, NPC
> hide-added-stat behavior, populated same-EXE captures, DPI and human visual
> comparison remain open. `visualAccepted=false`, `accepted=false`,
> `globalParityPercent=null`.
>
> 2026-09-03 Windows-native Crystal item-tooltip checkpoint: the Candidate now
> renders the source eleven-section item hint rather than a compact substitute
> for authoritative inventory, belt, equipment, personal-storage and
> warehouse-side bag cells. It consumes exact/real item and recursive socket
> metadata plus live player stats, masks unidentified values, colours unmet
> requirements red, formats expiry/seal/rental .NET times, preserves disabled
> cell hover without actions, and anchors at Crystal cursor `+(28,28)` with
> viewport clamping. Client native-ui passes 509/509 and focused source/time,
> lifecycle, Simulation and game-data regressions pass.
>
> Same-process evidence uses EXE SHA-256
> `5257E859B4AB173A8076B58778C59D09D291A7EB90F0FCFA38F696E46181A56F`.
> The real-pointer F12 frame is
> `docs/generated/player-qa/native-ui-parity-20260903-item-tooltip/item-tooltip-in-game-1788370099170-2.png`
> (SHA-256
> `82F2D5ACAB20874FB31D3C3B3EF8EA105D03495BD32ACED27D7BDE0EE6B78AB`)
> at `BichonProvince (288,616)`; it visibly preserves the exact WoodenSword
> field order after the item operation layer is dismissed. NPC/cash shops,
> quest rewards, guild storage and trade remain open because their compact
> models do not yet carry full authoritative metadata. Trusted package/light
> provenance, DPI and human comparison also remain open.
> `visualAccepted=false`, `accepted=false`, `globalParityPercent=null`.

> 2026-09-03 Windows-native InventoryDialog movable checkpoint: Crystal
> declares the `Title/196` root `Movable=true` and `Sort=true` in
> `InventoryDialog.cs:25-31`; generic movement records the press offset,
> follows the pointer, clamps the true window size to the stage and stops on
> release in `MirControl.cs:852-935`. Native now keeps a persistent
> `InventoryDialogUi` position, starts only from parent-owned background
> pixels (not tabs, Add, Close, Gold/free-count/delete controls or any of the
> forty item cells), preserves placement across Hide/Show, and clamps the
> source `316x236` frame to `(0..707, 0..531)` at 1024x768. The input path also
> retains the pre-press cursor so Windows SendInput/high-polling event batches
> cannot mistake the drag destination for its source.
>
> Original `Title/196.png` is `316x236` (SHA-256
> `987ACE9AA582868FF589DD923C64109E8D883549C9B80FE72ED7AFD981A0CB3B`).
> Final current-tree r5 EXE SHA-256
> `F08FC69F744BC9D6895A7756CF98AAF5A69EEEF4CA8AF10F65BA78D8663B33D3`
> produced same-run/same-character/same-world before and after evidence at
> BichonProvince `(287,616)`: origin
> `docs/generated/player-qa/native-ui-parity-20260903-inventory-drag-final/inventory-drag-inventory-1788365024393-1.png`
> (`64F8A6A4ECB500F40C508ED804FDA686DEEF11A4FDAF507601680202064A5B5C`)
> and moved
> `docs/generated/player-qa/native-ui-parity-20260903-inventory-drag-final/inventory-drag-in-game-1788365096792-2.png`
> (`AED8F77D8F8F6A43E80F985A13748ECFD11BA26C33B462A406F01F6185C8CFE3`).
> Their sidecars freeze `inventoryLocation=0.00,0.00` then
> `275.00,208.00`, with the same run id, panel, page, item state and world
> coordinate. Drag regressions pass 4/4 and the complete native-ui suite
> passes 496/496. This closes movable placement only; live stacked-item
> amount evidence, locked/sealed overlays, the full source item tooltip,
> authoritative light/trusted package provenance and human comparison remain
> open. `visualAccepted=false`, `accepted=false`,
> `globalParityPercent=null`.

> 2026-09-02 Windows-native InventoryDialog precision/DeleteMode checkpoint:
> the three source pages now keep Crystal's exact cell geometry, draw item
> icons at their exported size centered in the cell, show stack count only
> when greater than one, and keep QuestInventory read-only. The footer follows
> `InventoryDialog.cs:134-193,384-427`: Gold at `(40,212)`, the `84x6`
> `Prguse/24` weight bar at `(182,217)`, free slots at `(268,212)`, and the
> delete control at `(291,212)`. DeleteMode now uses source ButtonA/ButtonB
> sounds and steady frame `Prguse2/368`, right-click cancellation, the
> top-centred `Prguse2/366` cursor, Crystal's `204x109` `Prguse/238`
> `MirAmountBox` for stacks, and the `456x190` `Prguse/360` Yes/No message box
> for a single item. Confirmed requests cross the native protocol as the exact
> item instance/count `DeleteItem`; pending-operation correlation and stale
> stack/replacement guards release only on the matching authoritative receipt.
>
> The current-tree EXE (SHA-256
> `61F053C88FB4BCFF6C6BE0FB4A43C1AE8C807437986B3289D3A55536CC5EFF26`)
> produced the Quest-page precision capture
> `docs/generated/player-qa/native-ui-parity-20260902-inventory/inventory-final-inventory-1788360618712-1.png`
> (SHA-256
> `28FCCB8F7E7C0061823C9C3F494F489E04A7C4A79BC2FF5CA60A981498F087D3`)
> and, through a real Gateway disposable four-item character, the non-
> destructive single-item confirmation capture
> `docs/generated/player-qa/native-ui-parity-20260902-inventory-delete/inventory-delete-final-inventory-delete-1788362950757-1.png`
> (SHA-256
> `50F30B959365E5E2F4B2F6C1BA677118A683A4A7E91E47FEB4E5E6953F8761B1`).
> YES was deliberately not pressed, so evidence creation deleted no item. The
> full native-ui suite passes 492/492, and focused Windows capture, protocol,
> bridge and receipt regressions pass. The stack amount path is exact and
> automated but still lacks a live authoritative stacked-item screenshot;
> movable panel placement, locked/sealed overlays, full source tooltip,
> authoritative light/trusted package provenance and human comparison remain
> open. `visualAccepted=false`, `accepted=false`,
> `globalParityPercent=null`.

> 2026-09-02 Windows-native CharacterDialog source-parity checkpoint: the
> Character page now uses Crystal's `264x380` root at `(760,0)`, male/female
> `Prguse/340|341` page, `Title/500..503` tabs, `Title/504` name bar,
> `Prguse2/360..362` close button and all fourteen source equipment-cell
> rectangles. The renderer consumes authoritative `UserInformation`
> gender/hair/guild/class data and equipment `ItemInfo.Image`, resolves exact
> `StateItem` width/height/x/y metadata, and composes armour -> weapon ->
> helmet-or-hair with Crystal offsets. The native asset gate now requires the
> exported `StateItem` closure (214 metadata frames and 214 PNGs) plus the
> gender/hair frames; the manifest asset hash is
> `f5b49eef701a32209a0bd2bfdeecac202110f6413acdc07708eafe5d598666b5`.
>
> Auto-capture can now open Character through the real native UI reducer and
> waits until name, map, max HP and self position are authoritative. It also
> closes the login notice through `NoticeDialogState::close`, so aligned
> evidence cannot pass by racing an incomplete startup frame. The latest
> linked current-tree EXE is SHA-256
> `996CF54AF6A2560EECFA03E87296EDFF797FE3B59C7400DCEB453B53A86C3656`;
> its clean 1024x768 `1231` capture is
> `docs/generated/player-qa/native-ui-parity-20260902-character/character-clean-character-1788359861663-1.png`
> (SHA-256
> `DED8B034522963FDDD68913F5A41B7F6E2DBF02FBCE7FCC3C7702BCB6062DE36`).
> The paired sidecar freezes `panel=Character`, map `BichonProvince` and
> `(290,620)`. Focused Character geometry/composition, StateItem projection,
> asset-gate and capture suites pass, including capture 17/17. This remains
> draft implementation evidence: the sidecar lacks authoritative light and
> trusted package provenance, Crystal wing state is not yet available to the
> Candidate, the remaining Character tabs are not fully source-complete, and
> no human visual acceptance is recorded. `visualAccepted=false`,
> `accepted=false`, `globalParityPercent=null`.

> 2026-09-02 Windows-native Quest interaction source-parity checkpoint:
> Crystal's four distinct Quest surfaces now have source-shaped native paths
> instead of being collapsed into one custom panel. The NPC Quest List uses
> the source `316x466` geometry at `(487,0)`, `Prguse/950`, `Title/14`,
> `Prguse/951|957`, `Title/270|273|276`, and `Title/530`; it lists only quests
> exposed by the current NPC and accepts the runtime's real
> `@quest:accept:<id>` / `@quest:finish:<id>` links (plus the legacy Crystal
> forms) while retaining the exact NPC object-id gate. Diary secondary-click
> now toggles a stable, ordered maximum of five tracked current quests; the
> tracker is frameless at source `(0,100)`, includes every objective line, and
> has no invented implicit fallback. Detail CANCEL now opens Crystal's
> `456x190` `Prguse/360` message box with `Title/206` YES, `Title/210` NO and
> exact copy `Are you sure you want to cancel this quest?`; abandon is emitted
> only after YES, while NO/Escape preserves the quest and all independent
> Quest windows.
>
> A rebuilt current-tree Windows EXE (SHA-256
> `5A6D52FEB89949E23CA177D5A56FC24069D18CBD69C8BB6E2E7E55790BC2C099`)
> exercised the real local Gateway path with a fresh isolated QA character.
> Blacksmith Smith's quest 5 was accepted through `@quest:accept:5`, the NPC
> marker advanced from available `!` to current `?`, Diary showed `In
> Progress`, right-click produced `Kill Deer (0 / 10)` and `Kill Scarecrow (0 /
> 10)`, row left-click opened independent Detail, and CANCEL -> NO left the
> quest and tracking intact. Renderer-owned evidence is
> `docs/generated/player-qa/native-ui-parity-20260902/quest-ui-q2-in-game-1788354007393-1.png`
> (NPC List visual, SHA-256
> `F8401924BFED5AEF0764C287EEC30340D48EBD0063E0F32D0AE1213B96291679`) and
> the q3 captures `quest-ui-q3-in-game-1788354708271-1.png` (Diary + tracker,
> `789449D4683A38DB272A1CB078279EF2172C4EBE114F420B3A762EC74D8DC4D9`),
> `quest-ui-q3-in-game-1788354757568-2.png` (exact CANCEL confirmation,
> `46FC9EDF6FB6A8706236900375B09B66FFB0421D6ADF6E2B2DBBB410C9BD68BE`),
> and `quest-ui-q3-in-game-1788354839986-3.png` (NO-preserved stable state,
> `7D3EC23D6C17BC79BEEA595276C1D0E3C4A5AC8C5FD6D120CEDEA8EC6665F0B8`)
> in the same directory. Verification passes Quest UI 52/52, Rust fmt,
> current-tree Windows build, original asset-manifest regeneration (40,877
> assets; asset hash
> `3d6b7f125a91121ccde5b9a2db5dfa4faba5e29be2f27bc4bcfc62a086ec45e4`),
> real mouse/keyboard interaction, visual inspection and targeted
> `git diff --check`. The generic NPC conversation body remains visibly
> non-source-shaped; credit-reward/item-frame fidelity, hover/pressed animation,
> tracker persistence, capture-sidecar coverage for NPC List/tracker/message-box
> state, trusted same-EXE comparison and final human acceptance also remain
> open. `visualAccepted=false`, `accepted=false`,
> `globalParityPercent=null`.

> 2026-09-02 Windows-native Quest Detail source-parity checkpoint: the
> source-shaped Diary checkpoint below now opens a separate Detail window on
> row left-click. Detail uses Crystal's `(532,60)` geometry and original
> `Prguse/960`, `Title/16|17|616..618`, and
> `Prguse/919|965|966|979|989` assets, source scroll controls, 16-line message
> model, description/task/return/completion/time fields, fixed/select reward
> metadata, and SHARE/CANCEL command gates. Closing Diary leaves Detail open,
> matching Crystal. Gateway and Windows adapters retain the required quest
> fields, `shareQuest` reaches the camel-case native wire command, and both
> current-quest states can request abandon as Crystal's server permits. Final
> renderer-owned evidence against an isolated import of Crystal character
> `1231` is
> `docs/generated/player-qa/native-ui-parity-20260902/quest-detail-final-in-game-1788351341163-1.png`
> (Diary + Detail, SHA-256
> `2F733E4EEC558B2C9D52B2D09A906AC6B56889ECA01A450258BBE0480B452B59`) and
> `docs/generated/player-qa/native-ui-parity-20260902/quest-detail-final-in-game-1788351355082-2.png`
> (independent Detail, SHA-256
> `2B7F52B46F439F44B88AAB0F6A7C5700B244CC710FECBA7EB520B5DABAE00A01`).
> Their draft sidecars bind BichonProvince `(290,620)` and respectively record
> `panel=QuestLog;questDetail=Some(1)` and
> `panel=None;questDetail=Some(1)`. Verification passed native Quest UI 45/45,
> Windows definition adapter 1/1, native command serialization 1/1, capture
> sidecar 1/1, Gateway contract 1/1, Windows/Gateway builds, visual inspection,
> original asset-manifest regeneration (40,838 assets), and
> `git diff --check`. Remaining Quest leaves include the exact CANCEL
> confirmation message box, NPC Quest List, right-click five-slot tracking,
> the locally absent Crystal credit-reward frame, exact MLibrary item-frame
> fit, trusted same-EXE comparison, and final human acceptance.
> `visualAccepted=false`, `accepted=false`, `globalParityPercent=null`.

> 2026-09-02 Windows-native Quest Diary source-parity checkpoint: the Q-key
> surface no longer reuses the `Title/670` Mail frame or presents invented
> All/Active/New/Done filters, inline detail and action controls. It now uses
> Crystal `QuestDiaryDialog` geometry and original assets (`Prguse/961`,
> `Title/15`, `Prguse/917|918|956|997`, `Prguse2/360..362`, and
> `Title/193..195`), starts at `(192,60)` in the 1024x768 client area, groups
> current quests in source order, expands groups by default, and renders the
> source `List: n/20`, `LvN` and `Complete`/`In Progress` labels. The Gateway
> and Windows adapter now retain `group` and `minLevelNeeded` instead of losing
> them between `NewQuestInfo` and the native read model. The canonical Crystal
> `1231` Wizard/Male state was imported into an isolated Candidate slot; live
> verification showed `List: 1/20`, `BichonProvince`, the tracked check,
> `Lv1 Assistant's Request`, and `Complete`. Renderer-owned evidence is
> `docs/generated/player-qa/native-ui-parity-20260902/quest-diary-in-game-1788348421931-1.png`
> with SHA-256
> `29ACF7E67FBA7726441556C1AD07B054D386DD99C09D6870C4D453BDF49B9C8F` and
> a truthful draft sidecar. Verification passed native Quest UI 43/43,
> Windows adapter 1/1, Gateway contract 1/1, Windows build, asset-manifest
> regeneration (40,827 assets), and `git diff --check`. This earlier checkpoint
> closed only the Diary main-surface leaf; the Detail/left-click leaves it
> listed as open are superseded by the newer checkpoint above. NPC Quest List,
> right-click five-slot tracking, trusted same-EXE comparison and final human
> acceptance remain open. `visualAccepted=false`,
> `accepted=false`, `globalParityPercent=null`.

> 2026-09-02 Windows-native Run lighting-camera closure: the shader-only
> `144x96` virtual guard described below was necessary but did not make a
> stationary light pass follow a moving presentation camera. The final defect
> was a frame-of-reference mismatch: `MainCamera` followed the committed
> display-Hz player pose while `MirLightingBufferCamera` and
> `MirLightingComposite` remained at the map-frame origin, turning the
> defensive `border_darkness` margin into the user-visible straight strip.
> The runtime now synchronizes both lighting followers immediately after
> `follow_player`, before publishing the presentation pose. A non-origin-camera
> regression preserves lighting Z/scale and locks the guarded composite size;
> shared runtime passes 211/211. Live 1024x768 Bichon QA on the current-tree
> Debug EXE exercised left, reverse-diagonal and downward-diagonal multi-cell
> runs against the local `7010` Gateway with continuous viewport edges and no
> fixed-width black band. The remaining soft night-light falloff is expected
> scene lighting. This closes this specific Run edge-exposure leaf only;
> broader movement feel and final human frontend acceptance remain open.
> `visualAccepted=false`, `accepted=false`, `globalParityPercent=null`.

> 2026-09-02 Windows-native user-reported dialog-parity goal, first bounded
> implementation checkpoint: Crystal source audit confirms that the reported
> Character mismatch is a missing paper-doll composition/data chain, Inventory
> is missing source footer semantics and exact item fit/delete behavior, and
> Quest incorrectly substitutes one custom Mail-framed panel for Crystal's
> distinct Diary/List/Detail/Tracking surfaces. The first Inventory footer
> slice now uses source coordinates for numeric-only comma-formatted Gold,
> free-slot count, the three source weight-bar assets/thresholds and the
> `Prguse2/366..368` bin artwork; `UI_32bit/470` and `471` were exported from
> the original library. The bin deliberately does not reuse DropItem: Crystal's
> cursor delete mode, stack quantity prompt, confirmation and DeleteItem flow
> remain an explicit open leaf. Focused formatting, geometry, weight/gold and
> fail-closed action tests pass. A current-working-tree Windows EXE
> (`C9B0CE3DD549D82251430A410CACCB8B192A38E3FA2D890CA46A3C1F4D4B4481`)
> produced the renderer-owned 1024x768 capture
> `docs/generated/player-qa/windows-visual-parity/ui-gap-inventory-current-working-tree/inventory-working-tree-in-game-1788335861573-1.png`
> (`0236FAA64D1FDF2A194154428E3E027A73667601827D11EA392DD0F19F97E847`).
> Its sidecar correctly remains `mir2-native-visual-capture-draft-v1` with
> incomplete authoritative state and unavailable trusted package provenance;
> it is implementation evidence, not same-state Crystal comparison or final
> acceptance. The submitted pair mixes seeded `demo/Scout` with Crystal
> character `1231`; canonical appearance/inventory/quest-state extraction and
> slot mapping are now a separate P0 QA leaf before those content differences
> can be scored. `visualAccepted=false`, `accepted=false`,
> `globalParityPercent=null`.

> 2026-09-01 Windows-native Run viewport-edge/performance checkpoint: the bright
> strip captured during Run came from a stage-sized world-space lighting
> composite moving with the presentation camera, not missing map bindings. An
> initial physical `1312x960` offscreen target fixed the strip but added about
> 60% light-pass pixels and was superseded after the user reported that the
> picture felt unable to keep up. Native now renders the light texture at the
> original `1024x768`; a `1312x960` composite mesh provides a shader-only
> `144x96` virtual guard whose central region is sampled 1:1 and whose exterior
> emits the exact ambient darkness. This covers the maximum Run presentation
> offset without the enlarged offscreen fill cost or screen-relative lights.
> Runtime tests pass 207/207, the locked Windows Debug build passes, and fresh
> live directional routes recorded five Run plus eleven Walk commands with
> 16/16 confirmed authoritative locations, zero correction/rollback,
> acknowledgement latency of 13-77 ms (31 ms average), and
> `missingBindings=0` throughout. The running Debug comparison retains the dark
> viewport-edge coverage. This closes the diagnosed edge-exposure/performance
> regression, not wider movement feel or final human frontend acceptance.
> `visualAccepted=false`, `accepted=false`, `globalParityPercent=null`.

> 2026-09-01 Windows-native building-opacity repair checkpoint: the user's
> `(294,616)` side-by-side proved that the remaining pale appearance was
> isolated to ordinary Middle/Front building slices, not map gamma. Crystal
> exports those `.Lib` frames with authoritative 0/255 alpha and draws them at
> opacity 1, while the native pack was applying a second black-key/feather pass
> that changed real dark roof and wall pixels to partial alpha. Local normal
> frames now use the same byte-for-byte RGBA staging as additive frames; runtime
> blend selection remains driven by Crystal map flags. Rebuilt map-0 manifest
> SHA-256 is
> `E82E5573E98BDFC65B7EF463C9F09585F12805399E0783F7837D95F2D0AC1B1D`.
> Visible regression frames `#7680/#7684/#7688/#7692/#7695/#7701/#8403/#8413/#8678`
> are byte-identical to source, the native pack test passes, and a fresh live
> 1024x768 observation at the same coordinate shows opaque dark buildings with
> no ground wash-through. This closes the diagnosed native alpha defect, not
> the wider map/UI/VFX denominator or human acceptance.
> `visualAccepted=false`, `accepted=false`, `globalParityPercent=null`.

> 2026-09-01 Windows-native map depth/building/movement checkpoint: the
> user-observed missing buildings were a crop/asset-completeness defect, not an
> intentional rendering difference. Native now applies Crystal's `16x17` map
> cell scan plus a 25-row Middle/Front bottom-anchor lookahead and fails closed
> unless the native keyed manifest is present. The rebuilt map-0 manifest
> (`E82E5573E98BDFC65B7EF463C9F09585F12805399E0783F7837D95F2D0AC1B1D`)
> records 7,672 refs / 4,703 entries / 4,520 keyed / 183 additive / 2,969
> current missing-source entries, includes the complete ten-frame
> `Objects#2723..2732` family, and produces live `tiles=1090`,
> `standalone=334`, `missingBindings=0`, `incompleteFamilies=0`. The 2,508
> figures below remain historical pre-all-phase baselines, not the current pack
> denominator. Native Debug no longer forces Day: absent a fixed override it
> uses Crystal's UTC dynamic light setting, with no gamma hack. Keyboard and
> pointer movement share NewMove/ACK/collision/prediction; released right-click
> paths continue to their destination, Run predicts the source-backed two/three
> cells, and server Walk/Run cadence is the real 600 ms. Focused Windows suites
> pass 42/42, 35/35 and 28/28; the pack Node test and shared Zone 204/204 pass.
> This removes the diagnosed automated blockers but does not close the wider
> map/UI/VFX denominator or human frontend acceptance. `visualAccepted=false`,
> `accepted=false`, `globalParityPercent=null`.

> 2026-09-01 Windows-native character-select regression checkpoint: the user's
> occupied-roster comparison invalidated the older broad WN-VIS-005 acceptance
> wording. Native Last Online data was not connected to durable logout metadata,
> its label/value typography drifted from Crystal, and loading preview frames on
> demand exposed transparent intervals as visible flicker. The bounded repair
> now carries UTC binary LastAccess from final/abnormal server save through
> Gateway into Web and Windows, restores Arial 8 pt and the source row bounds,
> retains all 16 preview frames, advances all layers from one clock only after
> every target layer is dependency-loaded, and implements Crystal's explicit
> `VerticalCenter` flag through a fixed 21 px flex container rather than a
> coordinate offset. Focused cross-layer tests and a
> 1024x768 local Debug comparison pass, including nonblank samples across a full
> animation loop. Historical empty-roster 100/100 evidence remains historical;
> this occupied-roster regression is only Candidate-repaired until the user
> accepts the running window. `visualAccepted=false`, `accepted=false`,
> `globalParityPercent=null`.

> 2026-08-29 Windows native login-notice implementation checkpoint: Crystal
> runtime `StartGame` now emits a project-owned `UpdateNotice` only after a
> successful gameplay entry; Gateway preserves the typed `{notice}` envelope;
> the Windows adapter retains a monotonic connection-scoped update; and native
> Bevy renders the source-sized `Prguse/961` panel with original close, scroll,
> position-bar, and OK assets. Focused Simulation, Gateway, protocol, adapter,
> and UI-state tests pass. Default startup chat fallbacks no longer inject the
> unrelated LOMCN/Suprcode/JevLomcn/Net.8 copy previously seen by the user.
> This is implementation evidence, not visual acceptance: exact same-EXE
> capture, clickable/color span fidelity, persistent Crystal
> `LastUpdate > LastLogoutDate` behavior, DPI, soak, and human acceptance remain
> open.

> 2026-08-29 Windows safe-zone regression repair checkpoint: revision
> `aae9c2c7e06dbceb6f6539c7b29eba63ece293c4` restores the imported Crystal
> server's `SafeZoneBorder=True` presentation path. The Simulation default now
> derives from generated `TrapHexagon` boundary objects instead of suppressing
> them, while an explicit opt-out remains. The existing Windows persistent
> effect renderer is source-bound to `Magic 1390..1399`, 100 ms frames and
> authoritative removal; `inSafeZone` is also retained in the shared read
> model. Full Simulation 1482/1482 and focused Windows/read-model tests pass.
> Exact-head packaging, timed capture and human comparison are still required,
> so this does not close global visual parity or authorize a percentage.

> 2026-08-29 Windows-native user-observed visual/UI backlog: the current
> native Candidate is explicitly not visually accepted. Recent side-by-side
> user checks against Crystal confirm several still-open gaps that must remain
> tracked as unfinished work instead of being implied complete by narrower
> automated leaves. Open items now include: an implemented but not yet
> exact-Candidate-accepted login-to-game notice popup;
> missing safe-zone indicator; substantial HUD/chat/button/layout drift versus
> Crystal; missing or incomplete hover tooltips on the right-side function
> strip, bottom-right action buttons, chat mini-buttons and inventory/item UI;
> inventory/belt slot sizing differences; item tooltip/detail gaps; missing
> Crystal-style monster/NPC cursor semantics in some remaining paths; native
> world rendering that still reads softer or blurrier than Crystal on some
> buildings; missing or unaccepted scene animations such as lamp flame/fire;
> an incomplete quest/quest-diary flow; missing NPC-head available,
> in-progress and turn-in guidance markers; and unresolved human-feel concerns
> around movement cadence, click-to-move, run responsiveness and interaction
> pacing. The right-side system menu, main-HUD buttons, and chat mini-button row
> remain a functional denominator, not merely an asset-presence check. These
> observations are retained as
> open Windows visual-parity backlog, not as evidence of completion, and they
> must be closed with exact same-EXE screenshots, bounded automation where
> possible, and final human acceptance before any 1:1 completion claim.
> The numbered denominator is recorded in
> `docs/generated/player-qa/windows-visual-parity/VIS-03-USER-OBSERVED-UI-RENDER-BACKLOG-20260829.md`.

> 2026-08-29 Windows-native P0 environment/quest-marker implementation
> checkpoint: Type100 tile-animation bytes now survive parsing; tile, middle,
> and front animation families are expanded only when every source frame is
> packaged, then advanced by a local 100 ms clock using Crystal's frame/tick
> dwell formula. Incomplete families fail closed to their stable base frame
> instead of flashing transparent. Runtime tests pass 205/205 and focused map
> parser tests pass 31/31. Separately, authoritative NPC `questIds` plus the
> native `QuestTracker` now select Crystal's real `Prguse` `983..988`
> question/exclamation pairs, advance them on the original 500 ms cadence, and
> honor ready > available > in-progress priority, with focused overlay tests
> passing 11/11. Neither item is visually accepted: exact NPC body-frame
> anchoring, complete quest state transitions,
> timed same-EXE fire/lamp captures, additive/blend fidelity, full animation
> asset coverage, soak and human acceptance remain open.

> 2026-08-29 Windows-native VIS-01 Type1 transfer regression checkpoint: the
> user-provided GroceryStore screenshot is an explicit failure. Map identity
> reached `0141` / `(5,12)`, but old Bichon pixels remained and player/NPC
> bodies disappeared. Two independent causes are now covered: the Candidate's
> native keyed-map assets were generated only from map `0`, omitting 307
> `WemadeMir2/Objects2` frames referenced by Type1 map `0141`; and the map
> boundary cleared the local actor presentation along with source-map actors.
> The asset builder now parses Type1, builds `0,0141`, verifies immutable Full
> Crystal content/library/page hashes and crops all 307 required frames with
> exact source offsets. Windows render handoff identity includes the map file,
> unavailable destination rendering clears stale terrain, and scene reset
> preserves only the authoritative self actor while dropping old remote
> population/transients. Native keyed-map tests pass, a real combined build
> reports 4,957 emitted / 307 Full-Pack-derived / 2,508 known map-0 baseline
> missing, and Windows passes 474/474. Exact clean Candidate
> `WN-CANDIDATE-VIS01-TYPE1-MAP-TRANSFER-20260829` binds revision
> `be6eed8d3767e4381f064f957413564e4cb78df0`, passes packaging and independent
> final-directory verification, and is running as PID 290000 for the user's
> GroceryStore retest. It has not yet been visually accepted. Other maps,
> complete map/UI/VFX/actor denominators, live WSS, DPI,
> soak, human acceptance and formal publisher signing remain open;
> `globalParityPercent=null`, `visualAccepted=false`, `accepted=false`.

> 2026-08-29 Windows-native VIS-01 direct-frame/run/chat follow-up: the user
> visually rejected `02bb67874791c26e556fee88382d0e7d61287012` because the
> actor still flickered; stable `TextureAtlasLayout` identity was therefore
> insufficient. Revision `a3121ce487c93ff37f2ca94d7d60d8e12bf9e5ea`
> removes ordinary animated sprites from the dynamic layout/index path and
> updates a direct source `Sprite.rect` on one retained full-page image. Atlas
> pages remain cached across animation page changes; additive UV-material
> rendering is unchanged. The same revision fixes the extensionless chat frame
> lookup (`original-ui/Prguse/2221.png`) and adds right-click empty-world Run
> intent while leaving Zone validation authoritative. Shared runtime 197/197,
> native-UI 430/430 and Windows 448/448 pass. Exact clean Candidate
> `WN-CANDIDATE-VIS01-DIRECT-RECT-RUN-CHAT-20260829` passes final nonvisual
> verification; its 67,430,912-byte EXE is SHA-256
> `4EB134ABDA3CC4981A4268CF4501E2ABB5BEDCD3E1C0F2E23F653008C7F8D57A`.
> PID 243288 is running against healthy plaintext loopback for a new user
> check; no visual pass is recorded yet. Full mouse combat/click-to-path,
> complete chat/UI, alternate classes, skills/VFX, map/monster/player semantic
> denominators and the authenticated WSS/DPI/soak/human/publisher-signing gates
> remain open. `globalParityPercent=null`, `visualAccepted=false`,
> `accepted=false`.

> 2026-08-28 Windows-native VIS-01 exact-head motion Candidate evidence:
> clean revision `94f8e4f032643fdb826d6ef0ae82b360b4dcc83d` produced Candidate
> `WN-CANDIDATE-VIS01-MOTION-20260828` from its attested 67,435,520-byte EXE
> (SHA-256
> `E40C5216A29DE870DA7898F0ACABE331E7310C583D249C2F66DC3210692050F4`).
> The 32,594-file Candidate passes detached-CMS and final nonvisual
> verification; payload aggregate SHA-256 is
> `167EB82528CD5ADEDA5621B170233FA2B8314540F11A95E65EB812ECA8D5B726`.
> This adds exact-head artifact evidence to the mounted-cadence and continuous-
> locomotion leaves, but no visible acceptance: the EXE was not launched and
> native keyed-map generation still has 2,508 missing source entries.
> Same-EXE live WSS, mounted input, UI/chat, mouse combat, skills/VFX,
> complete monster/map denominators, DPI, soak, human acceptance and formal
> publisher signing remain open. `globalParityPercent=null`,
> `visualAccepted=false`, `accepted=false`.

> 2026-08-28 Windows-native VIS-01 mounted cadence/packet-clock bounded
> checkpoint: revision `eb174e94eecde4a6e24f63d16616e2dfb9a03589`
> closes the source/test portion of the mounted timing gap left by the prior
> locomotion checkpoint. Windows now derives player movement phases from the
> active Crystal catalog: mounted Walk is 8 x 100ms and mounted Run remains
> 6 x 100ms. The same phase count drives pixel displacement, visible frame,
> motion duration and self-camera cancellation. Active packet-carried start/
> end timing is preferred over receipt time; invalid metadata falls back
> without freezing or extending the actor, and explicit counts are bounded to
> 1..8 like Web pose input. Focused 5/5 and Windows 446/446 pass; independent
> reviews find P0=0/P1=0. No exact-head EXE/package, actual mounted input,
> screenshot, DPI, soak or human-feel evidence was produced. Complete actions,
> UI/chat, mouse combat, skills/VFX, monsters and final device/human/signing
> gates remain open; `globalParityPercent=null` and
> `visualAccepted=false`.

> 2026-08-28 Windows-native VIS-01 continuous player-locomotion bounded
> checkpoint: revision `532ddc6be0a0c38313fdd39fe9e0af82b883371b`
> aligns ordinary overlapping Walk/Run presentation with the already-working
> Web model: the next segment starts from the current fractional coordinate,
> its animation sequence/phase starts atomically with the native motion
> window, and a direction/source-bounded self echo cannot roll actor and
> camera back. The opt-in runtime replacement applies only to player
> locomotion with an empty queue, so waiting Attack/Struck/Die actions retain
> FIFO semantics; monster/NPC presentation is excluded from the visible-frame
> override. Runtime passes 194/194 and Windows passes 443/443; independent
> player and Web/Crystal reviews report P0=0/P1=0. No exact-head EXE/package,
> live screenshot, DPI, soak or human-feel evidence exists for this revision.
> Mounted Walk still needs its Crystal eight-phase window and Windows still
> does not consume packet-carried movement start/end timing. The complete
> player/UI/VFX/monster semantic denominators and final device/human/signing
> gates remain open; `globalParityPercent=null` and
> `visualAccepted=false`.

> 2026-08-28 Windows-native VIS-01 player-sprite geometry/package bounded
> checkpoint: revision `7fa5369bcb6767ad5f1d1e1e0f07cac6bae8f7a6`
> replaces the repository-only 48x64 atlas-miss placeholder with exact
> `meta.json` geometry for canonical frames in the ten packaged player
> families. Atlas pages remain preferred. Verified standalone player frames
> retain transparent-pixel hit testing and atomic 30% selected/hover redraw
> through a bounded 256-frame alpha cache; malformed identity, missing PNG or
> metadata, half-atlas bindings and every non-player miss fail closed. This
> narrowly supersedes the earlier selected-target note that every missing
> atlas identity must suppress the clone: a complete verified standalone
> player layer may now be cloned, while partial or non-player composites still
> suppress. Package/verify close 10 families / 34 libraries / 22,944 declared
> frames, Windows passes 441/441, and release-gate revision
> `ef25aec83b8023003ae648b4a2955a4e9ec76362` produces a clean nonvisually
> verified Candidate with EXE SHA-256
> `1550B512930C54BA5356100B63976919A146E904F9A397D4EDE4CF653200FC3A`.
> This does not close missing source-frame denominator differences, complete
> player actions, UI/VFX, same-EXE live WSS, DPI, soak, human or publisher-
> signing gates. `globalParityPercent=null` and `visualAccepted=false`.

> 2026-08-28 Windows-native VIS-03 Character HUD additional bounded
> checkpoint: revision `849f1f0b5120867d1358e0e7db9ba675e9866f9c`
> verifies exact `Prguse/1900/1901/1902` at `(905,692,20,20)`, enabled
> pointer-only ButtonA ordering and the source CharacterPage-aware callback.
> A non-Character tab stays open and returns home; a visible CharacterPage
> closes. C/F10 share the state machine while remaining silent and local.
> Bevy 401/401, Windows 376/376, focused 4/4 and script self-tests pass; final
> independent review is P0=0/P1=0 after fixing the initially found keyboard
> gap. No package, EXE, live audio or screenshot was produced. Character
> content, other controls, actual scaled hitboxes, same-EXE pixels/audio and
> human feel remain open, so this is not HUD, UI or visual 100% and
> `globalParityPercent` remains null.

> 2026-08-28 Windows-native VIS-03 Inventory ButtonA additional bounded
> checkpoint: revision `5b70511316b084ac677b5978f7f03e440241ca4c`
> closes automated behavior for one local HUD interaction only. The enabled
> InGame Inventory mouse press queues Crystal `ButtonA=10103 -> 103.wav`
> exactly once before toggling the panel; held presses do not repeat,
> subsequent presses do, and F9/I remains silent. UI/gameplay queues and
> spawned-player cleanup are independent; missing source, disabled sound and
> zero volume never fall back. The exact 26,546-byte sound is package/verify
> required and SHA-256 bound. Windows 376/376, Bevy 397/397, focused 4/4 and
> script self-tests pass, with final review P0=0/P1=0. No live audio,
> same-EXE capture or package was produced. Character and every other
> control's source binding, hover/pressed/disabled feel, real DPI and human
> acceptance remain open, so this is not HUD, UI or visual 100% and
> `globalParityPercent` remains null.

> 2026-08-28 Windows-native VIS-01 selected-target additional bounded
> checkpoint: revision `a58ab0aaa2202731a5c55e7a684261d6c15c2f8d`
> redraws an explicitly selected remote player or monster's complete resolved
> exact-atlas composite at Crystal opacity 0.3 after world rendering. Missing
> atlas identity suppresses the whole clone; self/NPC/missing-kind objects do
> not highlight, dead monsters remain eligible, Hidden and selected opacity
> are independent, and Scarecrow's additive death effect is not duplicated.
> World, target and foreground-effect depth bands now preserve ObjectEffect,
> default MapEffect and actor/transient effects above the redraw while
> Persistent ObjectSpell remains in-world. Windows 376/376, Bevy 393/393,
> runtime 191/191, selected 3/3 and depth 1/1 pass; final review found no
> P0/P1. No EXE/package/screenshot was produced. Hover highlight, the setting
> toggle, general DrawBehind export, special composites, Web symmetry and all
> live/GPU/DPI/soak/human/signing gates remain open; global parity stays
> unreported.

> 2026-08-28 Windows-native VIS-02 GreatFireBall additional bounded
> checkpoint: revision `9457e5618449d22350baedd01e3775f5b1fe59c6`
> implements immediate `Magic/400..409`, a 600 ms delayed 16-direction local
> missile, bound `Magic/570..579` impact and exact M34-0/M34-1/M34-2 audio.
> The compatibility projectile is ignored and lifecycle cleanup fails closed.
> All 90 previously absent direction PNGs plus their source metadata and exact
> WAVs are now committed and required by package/verify. Windows 372/372,
> Bevy 393/393, focused 5/5, Gateway 1/1, Web full logic/type, exporter,
> offline resources and script self-tests pass; review has no remaining P0/P1.
> No EXE was launched or packaged from this head. Retained-dead target
> suppression, live WSS, same-EXE/GPU/DPI/soak/human/signing and the semantic
> denominator remain open; global parity stays unreported.

> 2026-08-28 Windows-native VIS-02 FireWall bounded checkpoint: revision
> `f6f78f3eddb813897cf4ce4c6056183130ab7f35` binds the source 600 ms
> `Magic/1620..1629` cast, exact M39-0/M39-1 phase sounds and five persistent
> center/cardinal `ObjectSpell` cells using repeating `Magic/1630..1635` and
> light 3. Native 351/351, Bevy native-ui 393/393, focused FireWall 5/5,
> Gateway projection 1/1, Web type/export/offline asset gates and package/
> verifier self-tests pass. The fixture is serializer-only, with `cast=false`
> labeled synthetic outside the canonical timeline; no EXE was launched or
> packaged from this head. FlamingSword remains open in the first five-skill
> slice, along with the full combat chain, backend negative/lifecycle matrix,
> denominator and same-EXE/GPU/DPI/soak/human/signing gates. Global parity
> stays unreported.

> 2026-08-28 Windows-native VIS-02 SoulFireBall bounded checkpoint: revision
> `19991af6ddb289dc2fb22569849599caabf9195e` implements the source audio-only
> start, 600 ms local launch, all 16 three-frame missile directions, launch-
> time target binding/direction lock, finite target-following flight, bound
> impact and exact M64-0/1/2 assets. Compatibility `ObjectProjectile` is
> ignored in every replay order. Native 346/346, Bevy native-ui 393/393,
> focused SoulFireBall 6/6, FireBall regression 11/11, Gateway event projection
> 1/1, Web/export/resource and package/verifier self-tests pass. No EXE was
> launched or packaged from this head. The projection fixture is serializer-
> only; production no-amulet `cast=false` is not proved. Target-dead impact
> suppression, backend Soul timing/revalidation/PvP gaps, FlamingSword,
> FireWall, the full denominator and all same-EXE/GPU/DPI/soak/human/signing
> gates remain open; global parity stays unreported.

> 2026-08-27 Windows-native VIS-02 FireBall bounded checkpoint: revision
> `d85d7368119053e6b2609316c4f5c76faaa298cb` now renders the source cast,
> 16-direction finite missile and bound impact from typed `ObjectMagic`, with
> exact M31-0/1/2 audio and fail-closed package/verify frame closure. The
> simulation compatibility projectile is deduplicated. Independent final
> review found no P0/P1; effects pass 59/59, Windows 340/340, Bevy native-ui
> 393/393, Gateway fixture 1/1, Web typecheck and the complete offline resource
> gate pass. No EXE was launched and no GPU/same-EXE/DPI/human evidence was
> created. Target-dead impact suppression, FlamingSword, SoulFireBall,
> FireWall, the full semantic denominator and every final acceptance gate
> remain open; global parity stays unreported.

> 2026-08-27 Windows-native VIS-00 baseline / VIS-01 in-progress checkpoint:
> the new visual contract binds
> the current implementation base to Crystal source and records known Phase-A
> registries without manufacturing a global percentage. The first repair
> routes text through Arial, applies 8pt logical sizing to chat/nameplates, and
> adds Crystal's exact four-pass black MirLabel outline, ordinary NameView
> alive-only labels, Hidden alpha 0.5 and ordinary corpse alpha 1.0.
> Gateway-to-native projection now retains remote class/gender/equipment and
> normal/Transform body metadata; Harvest and Harvested packets drive Harvest,
> CWeapon/01 and persistent Skeleton actions. The first VIS-01 increment now
> recognizes the real no-sprite `ObjectMonster(image=10)` packet, plays
> CannibalPlant `Monster/010` Show/Hide frames, hides only after completion,
> restores on Show and avoids applying that lifecycle to other monster types.
> Runtime 187/187, focused Windows 6/6,
> typography 1/1 and Gateway projection 1/1 pass. The full Windows suite
> against frozen Candidate assets is FAIL with 320/322 passing: the package
> lacks the asserted Archer and Mount frames. No new same-EXE screenshot was
> taken, so none of these leaves is visually Accepted yet; HUD/damage text
> sizing/bold, hover corpse names, Scarecrow additive death, real-map
> occlusion, the fixed VIS-01 scene/captures, additive weapon/wing layers,
> actor/effect
> assets, VFX/audio, UI states, DPI, soak and human feel remain open.

> 2026-08-19 Windows-native alternate-class/combat checkpoint: Archer
> `ARArmour`/`ARHair`/`ARWeapon` and Assassin
> `AArmour`/`AHair`/directional-dual-`AWeapon*` resolution now follows the Web
> library rules for their supported action families. Crystal combat packet
> adaptation now correctly separates `ObjectStruck` pose state from numeric
> `DamageIndicator` events; native overlays deduplicate and animate bounded
> hit/miss/crit/heal floaters. Stale server selection can no longer make the F
> key ignore a nearer live hostile. Live proof records Deer HP
> `10/25 -> 9/25 -> 8/25` and a visible red `1` in
> `native-windows-combat-overlay-round4/...1787122701585-1.png` (SHA-256
> `0E24F11B963382F02C82F0DAEEE745F51794A01460175E92523AABE7DCBD49AA`).
> Gates are Windows 104/104, runtime 133/133, Simulation 1183/1183, Release
> SHA-256 `B6A7078173865DF3415B089DE4119EAA438886EF518AFD9DEC69054B445773D9`,
> and Web typecheck. No starter entity record exposes a usable mask path or
> nonzero shadow tuple; shadow/effect parity is therefore an exporter/pipeline
> gap. Spell/projectile effects, lighting, exact text/overlap, broader class
> action audit and final same-scene/Gemini/human acceptance remain open.

> 2026-08-19 Windows-native entity-composition checkpoint: the producer now
> consumes 697 valid Crystal per-library frame-set catalogs with exact
> `start/count/skip/interval/reverse` metadata and Web-equivalent action
> fallbacks. Stable layer construction covers body, hair, directional front/
> rear weapon and mount composition, including mounted weapon suppression.
> Native Bevy overlays consume the same authoritative entity payload for NPC/
> monster/player labels, dead-state lines and the self HP bar. A live visible
> run moved from `288,615` to `287,613` with actor/nameplate/camera/minimap
> agreement; evidence SHA-256 is
> `343B4B05A9E67EF7B687F0DFA9B5D8D2F34E222A7AD95BC03AD6D1E4569E8DC4`.
> Windows 98/98, shared runtime 133/133, exact Release build and Web typecheck
> pass. Remaining entity gaps are alternate class libraries, shadow/effect-mask
> layers, combat effects, exact outlined text, overlap policy, lighting and
> final same-scene/Gemini/human acceptance.

> 2026-08-19 Windows-native animation-clock checkpoint: packet-authoritative
> walk/run/attack/range/magic/struck/die/revive hints now drive a persistent
> Windows-main-thread `AnimationWorld`; atlas rects continue advancing between
> Gateway messages and duplicate action sequences do not restart them. A 254 ms
> F12 pair changed 5,087 world-only pixels (rows 60..532), and live revive plus
> movement returned to `18/18 @ 288,616` then `288,615`. Windows 90/90, shared
> runtime 133/133, Release and Web typecheck pass. Remaining entity gaps are now
> explicit: schema-v2 per-library frameSet metadata, correct class/equipment
> libraries, body/hair/weapon/mount/shadow composite layers, nameplates/HP bars,
> high-priority interruption semantics and final same-scene visual review.

> 2026-08-19 Windows-native map/entity/vitals checkpoint: schema-v2 entity
> atlases now route rects to all seven physical pages and preserve Crystal
> `offsetX/offsetY`; packet-first `UserLocation` updates the real-map render
> center, shared HUD coordinates and entities in one frame; and map atlas page
> layouts are rebuilt when later camera viewports introduce additional rects,
> eliminating the reproduced black floor rectangles. Native self vitals now
> fold packet-authoritative `ObjectHealth`/`Death` over stale personal snapshots.
> A real dead save rendered `0/18`, `V` completed TownRevive to `288,616` at
> `18/18`, and later damage rendered `17/18` then `16/18`. Windows 83/83,
> runtime 133/133, Release and Web typecheck pass. The complete offline asset
> staging script also passed with 8,325 files / 269.91 MiB and all required
> sentinels present; clean-checkout generation and repository-independent EXE
> launch are not yet proven. Open
> gaps are now narrower and explicit: full actor action/equipment/hair/weapon
> composition, nameplates/health bars, lighting/effects, deterministic exact
> `257,594` evidence, final HUD >=92, DPI execution, packaged-EXE launch and
> human acceptance.

> 2026-08-19 Windows-native HUD Candidate checkpoint: the native-only Bevy
> presentation now uses Crystal `Prguse` MainDialog, bottom-clipped HP/MP orb,
> horizontal belt, chat control/four-line frames, main buttons, and the Bichon
> `MMap/101` crop with authoritative markers and coordinates. The fixed HUD-only
> evidence review is Accepted 88/100 with `sameScene=true` and no P0/P1, clearing
> the planned first-Candidate threshold of 85. Stable live evidence shows
> authoritative `NativeHero`, HP/MP, Bichon title and coordinates after bootstrap;
> the legacy fixed Combat Target panel is suppressed and the quest tracker uses
> Crystal-style transparent top-left text. Shared Bevy native-ui tests pass 90/90,
> Windows host tests 80/80, Release builds, and Web typecheck passes. Remaining
> HUD gaps are authoritative EXP/weight fields and bitmap-style character text;
> final HUD >=92, same-coordinate scene evidence, map/entity/effect coverage,
> 125%/150% DPI, package verification and human acceptance remain open. The
> model-reported minimap white shape is source artwork already baked into
> `MMap/101.png` at the candidate crop, not a renderer overdraw artifact.

> 2026-08-19 Windows-native visual implementation checkpoint: login and character
> select are no longer open visual gaps at the fixed 1024x768 gate. Login remains
> Accepted 100/100. WN-VIS-005 replaces the former 20/100 generic select card
> with Crystal's dedicated background, four slots, selected class frame,
> source-offset 16-frame class/gender preview, last-access row and five-button
> footer. Empty-roster same-scene review is Accepted 100/100 with zero issues;
> occupied-roster capture and real pointer StartGame additionally verify the
> preview and transition to authoritative Bichon. Windows map routing retains
> the Web-equivalent atlas plus black-keyed/additive standalone semantics.
> Remaining P0/P1 work is explicit: in-game is still 12/100, 2,494 referenced
> map source frames are unavailable, map animation/fixed-scene coverage is
> incomplete, and exact Crystal HUD/entity layering, effects, 125%/150% DPI and
> final human acceptance remain open. The next visual slice is the Crystal
> in-game HUD/chat/shortcut/minimap composition.

> 2026-08-19 Windows-native playable vertical-slice Candidate: the main branch
> now has a real Bevy/Winit/WGPU `mir2-platform-windows.exe`, not a WebView or
> browser wrapper. Its visible shell covers interactive login, account creation,
> server-backed character select/create/delete/start, and a native Bichon HUD.
> Gateway snapshots/packets drive NPC dialog, quest tracker, target HP, inventory,
> movement, combat, death/revive and completion; the client does not synthesize
> quest state, XP, gold or saved transforms. A fresh-account smoke completed q1
> and q2 in one execution, including 20 authoritative Scarecrow hits, the
> Crystal direct-Q `GainedItem(item_index=1112)` path, TownRevive, 30 quest EXP,
> 200 Gold, GoldenPendant/CopperRing, logout/login and persistence. Six fixed
> 1024x768 native screenshots plus the manual event checklist are indexed in
> `docs/NATIVE-WINDOWS-PLAYER-QA.md`. Regression gates pass at Windows 73/73,
> shared Bevy 22/22 without native UI and 56/56 with it, WASM WebGL2/WebGPU,
> Web typecheck/runtime-policy, release build and Crystal map API 18/18.
> This closes the native *playable slice*, not native Crystal 1:1: several
> starter entity atlas body/action frames have incorrect anchors or incomplete
> library coverage, the native HUD/chat/shortcut/minimap presentation is not
> pixel-matched to Crystal, 125%/150% DPI and long reconnect/physical-device
> feel still need human acceptance, and source quest text retains some malformed
> placeholders. Those are explicit frontend gaps rather than hidden behind the
> functional Candidate result.

> 2026-08-18 4K stage-overlay and Chinese presentation closure: a user-supplied
> 3840x2160 production capture showed that the 1024x768 Crystal stage itself was
> centered correctly, while Account Security and On-chain Mine were positioned
> against the browser viewport and therefore appeared in the black letterbox.
> Both overlays now portal into `.client-stage-frame`; their absolute position,
> drag bounds, compact layout and persisted-coordinate clamping all use the
> stage coordinate space. The Account Security trigger is anchored at the
> stage's top-left so it cannot cover the top-right language selector before
> login or the minimap after StartGame. The tutorial coach, debug toast, and
> death/revive backdrop now use that portal too; spotlight rectangles are
> converted from browser coordinates into the scaled 1024x768 stage space.
> The resource-cache progress/debug HUD is likewise stage-local, so map loading
> progress remains visible beside the game instead of in an ultra-wide letterbox.
> The same capture exposed English fallbacks in q3/q4,
> Accept, realm metadata, `BichonProvince`, NPC/monster nameplates and the mine
> panel. Presentation-edge localization now covers every q1-q9 state, structured
> quest NPC/reward names, the visible Bichon entities/map title, owner-Hero
> labels, realm metadata and the complete mine panel without mutating canonical
> packet/world identifiers. The importer and both generated bundles are kept
> byte-identical by regression coverage. Focused quest/localization,
> responsive-stage and mine tests, TypeScript, the direct Next production build,
> script syntax and diff checks pass. This is local Candidate evidence only;
> deployment plus an authenticated 4K Chinese screenshot remains the production
> and human acceptance gate.

> 2026-08-12 tracked-quest NPC marker closure: Gateway NPC packets already
> carried authoritative `questIds`, but the Web packet reducer discarded that
> field when creating/replacing live entities. Quest tracking could therefore
> succeed while the golden exclamation/question resolver saw no NPC binding.
> Packet ingestion now normalizes and preserves the binding, with explicit
> empty arrays still clearing it; Bichon simulation assertions cover Assistant
> Jane, Craft Lady Jude, and Blacksmith Smith. A fresh-account live capture saw
> Assistant Jane bound to quest 1, three quest icons, exact Crystal marker
> anchoring, one adjacent interaction, zero movement, zero console errors, and
> zero non-favicon 404s. The authenticated demo session also retained all three
> icons after tracking `简助理的请求`. This closes the local Candidate marker
> regression; production publication remains a separate release gate.

> 2026-08-11 main-based cross-platform client Candidate: the client-core and
> shared Bevy read-model/UI crates, native desktop/Android hosts, Tauri desktop
> shells, and Capacitor Android/iOS shells are integrated on top of the current
> main repository layout. Hosted gates cover shared Rust/WASM, Windows native
> assets, Tauri on Windows/macOS/Linux, Android APK plus native aarch64 compile,
> iOS simulator, and shell security contracts. This does not change the existing
> Crystal visual-parity score or close human Accepted status; physical mobile
> lifecycle/touch/thermal soak, store signing, and final human visual/feel remain
> separate gates.

> 2026-08-09 Bichon scene-cache black-floor closure: walking MIR4R1 across
> `293,610 -> 293,611 -> 293,612` changed the canonical scene chunk and exposed
> a poisoned v5 disk blueprint containing 138 explicit null back-layer
> references. Scene schema v6 now bypasses that entry, and disk/memory cache
> admission rejects null floor references, dangling sprite IDs, empty frame
> lists, and invalid frame dimensions. The exact v6 `cx18/cy36/w56/h72`
> request rebuilt once and then hit with 957 sprites, zero null back layers and
> zero dangling references. Focused cache/request tests and TypeScript pass; a
> real browser login plus StartGame had no critical console errors, and the
> live 610-to-612 boundary walk kept a complete floor with no loading-overlay
> recurrence. This is a local Candidate repair; deployment remains a separate
> release gate.

> 2026-08-02 mobile PWA/fullscreen shell closure: Player Web now publishes a
> standards-based fullscreen/landscape Web App Manifest with 192px, 512px,
> maskable, and Apple touch icons. Root metadata includes `viewport-fit=cover`,
> explicit iOS standalone/status-bar tags, and a dynamic-viewport page shell.
> The mobile-only install surface handles Android `beforeinstallprompt`, iOS
> Add-to-Home-Screen guidance, app installation/display-mode changes, and a
> user-gesture Fullscreen API plus advisory landscape lock fallback. Copy is
> available in English, Brazilian Portuguese, and Chinese; dismissal storage
> fails open in private browsing. It reuses the existing asset Service Worker
> rather than adding a competing worker. PWA contract/icon tests, responsive
> stage tests, TypeScript, HTTP manifest/metadata/icon probes, and the direct
> Next 16 production build pass. Automated local visual capture is not claimed:
> the in-app browser policy rejected control of the `127.0.0.1` tab. Physical
> iPhone and Android installed-mode login/wallet/fullscreen acceptance remains
> a human/device gate, separate from Crystal scene parity.

> 2026-08-01 Crystal map-weather/light override closure: generated map data now
> carries `map_dark_light` and `weather_particles` from `Server.MirDB`; typed
> `MapInformation`/`MapChanged` browser events preserve lights, dark tint, and
> weather bits; Web resolves fixed map light before global `TimeOfDay`, applies
> all five Crystal night tints, and renders Fog/Ember/Snow/Rain/Leaves variants
> from seven selectively exported `Weather.Lib` base frames. The layer stays
> lazy and compositor-bounded. Developer Compose and direct debug builds default
> to Day for readability; Release production remains UTC-dynamic, while named
> or numeric overrides retain deterministic Dawn/Day/Evening/Night QA.

> 2026-08-01 low-end rendering/resource closure: screen-staged prewarm removes
> character-selection and game/HUD packs from login blocking work, serializes
> later stages after Service Worker setup, and lets low tier skip optional audio
> and scene-frame scatter. Standalone WebGL2 map residency is now byte-bounded
> with visible-page pinning, LRU low-watermark eviction and explicit texture
> release on replacement, disable/Bevy takeover, context change and unmount.
> Cold atlas pages load before canvas clear so crossing a page boundary keeps the
> last complete frame rather than flashing transparent. The map shelf packer no
> longer rounds exact 4096 content up to 8192; all 40 generated pages are at most
> 1024x4096, and build/dev gates reject stale oversized manifests. Full frontend
> logic, TypeScript, focused tests and a forced-low live login pass with no
> browser warnings/errors. The live immutable 20260730 R2 release still contains
> the two old 8192 pages and requires a new release rather than an in-place
> overwrite. Physical 4 GiB Android soak remains the support gate; this resource
> work does not change the final human Crystal visual/feel gate.

> 2026-07-23 final deterministic frontend Candidate closure: the remaining
> fixed-font, chat-history, and actor-phase work is implemented. Eight exact
> Crystal acceptance strings are exported through Windows TextRenderer at Arial
> 8pt/96 DPI with source-accurate outline/background semantics and verified ARGB
> hashes; dynamic text uses the normal accessible renderer unless its complete
> key matches. Chat renders the original 17 types/colours, 614px wrapping,
> four-line history, filters and scroll position with no timestamps, driven by
> real shared Gateway broadcasts instead of capture-only startup strings.
> Persistent Rust animation state now owns each object's incarnation, seeded
> idle/harvest phase, FIFO action queue, death and revive lifecycle, and supplies
> one pose to the Bevy/WebGL2/DOM presentation paths. Game-screen lifecycle
> transitions reset the bridge so relogging cannot reuse a prior action queue,
> while an in-game network reconnect preserves visual continuity.
>
> Evidence `cwp-20260723-r40-gdi-chat-final` at Bichon `0 @ 328,275`, light 1,
> runtime `bevy-e9d354eada933661`, is 100% automated Candidate with 0 critical
> console errors and 0 non-favicon 404s. It records 6 exact GDI nodes, 12 Rust
> animation poses, the real `Online/LineMessage/Online/Online` four-line state,
> 89% world similarity, 88% full HUD, 91% HUD UI, 84% chat, and 87% MiniMap.
> Strict current WebGPU and WebGL2 movement captures remain clean, and the
> native/current-Web four-action report aligns all actions and emits 4/4 frame
> pairs. No automated P0/P1 frontend gap remains for this scene. The only open
> item is final human **Accepted** visual/feel review; 24% full-window and 26%
> world thresholded pixel change still includes different roaming actor
> positions, idle/effect sampling, and compositor output, so it is not described
> as bit-for-bit identity or as a movement defect.

> 2026-07-22 reproducible developer-handoff closure: the root repository now
> pins the maintained Crystal fork/branch, includes Windows bootstrap/start/
> verification entry points, and tracks an immutable private developer-bundle
> manifest instead of requiring an undocumented local asset tree. The full
> bundle contains the exact verified closure of one index, 1,440 library
> shards, and 4,446 unique PNG pages; its content hash is
> `f71b89aa38504c6c127b937043d4af6ecd26d9dd1a2b9ed3b91100e6a1f0052e`.
> Packaging is deterministic USTAR, installation rejects unsafe entries and
> performs a transactional swap, and remote release generation retains a
> source path plus SHA-256 for all 45,398 objects. Local release-doctor and R2
> upload-plan checks pass with exactly 5,887 full-pack objects. This closes the
> undocumented-code/assets handoff gap; it does not close the remaining human
> Crystal visual/feel acceptance or the not-yet-published full R2 endpoint.

> 2026-07-18 original q1-q9 frontend contract closure: quest dialogs now keep
> fixed rewards separate from mandatory q3/q6 selectable rewards and preserve
> item icon, template index, and selection index through Gateway JSON. The Web
> packet adapter, quest window, overlays, and objective tracker expose the
> original task progress and selected reward path instead of treating every
> reward as an automatic grant. Extended-packet 28/28, tutorial-flow 14/14,
> onboarding-guidance 17/17, stage5-adapter 68/68, TypeScript, and the Next
> production build pass against the completed simulation q1-q9 route. Final
> human dialog layout and route-feel acceptance remains open. Automated visual
> recapture was not claimed in this round because the in-app Browser rejected
> navigation from its post-restart error page when the existing tab URL
> contained a nested `ws://` query; the rebuilt local Web and Gateway health
> endpoints are green and the existing play tab can be refreshed manually.

> 2026-07-18 safe-zone TrapHexagon depth closure: persistent `ObjectSpell`
> effects were rendered at layer offset 72, above the entity offset 64, while
> Crystal sorts world spells before actors inside each map cell. Ground and
> world-spell bodies now use offset 48 and optional masks use 49; transient
> combat spells remain at 90. The live Bichon `0 @ 287,618` WebGPU scene kept
> all 52 visible TrapHexagon nodes on the exact Magic `1390-1399` loop with
> `plus-lighter` blending while restoring actor/NPC occlusion. The persistent
> effect path now decodes its deduplicated body/mask frame set before first
> display without adding per-frame DOM nodes; transient combat spells remain
> ungated. A cache-cleared first-entry MutationObserver measured all 52 beams
> with zero undecoded images at insertion, the next paint, and 100ms later.
> Unobstructed native/Web beam pixels were already matched, so opacity and
> source frames were intentionally left unchanged. `test:scene-effect-runtime`
> passes 10/10, `tsc --noEmit` passes, and the in-app 960x720 game capture has
> no missing beams or HUD blend leakage.

> 2026-07-18 Bevy run-camera transaction closure: the visible full-scene
> tremor was not a server rollback. Map/entity snapshots could commit a new
> center one Bevy frame before the local-command camera compensation, producing
> a zero-offset whole-cell flash. Consecutive commands could also rebase from a
> fractional prior pose while the TypeScript camera window used a neighboring
> phase, causing per-frame ownership to switch between `localCommand` and
> `selfWindow`. The runtime now reconciles camera/entity offsets in the same
> committed-center frame, preserves fractional motion-window coordinates across
> the JS/WASM boundary, and latches local presentation ownership across connected
> commands until a correction clears the segment. A live WebGPU run sampled 462
> display frames and five center changes with zero uncompensated center frames,
> zero active `selfWindow`/`static` samples, zero active source switches, and
> 5/5 matching command plus ACK diagnostics with no rollback. Evidence:
> `docs/generated/player-qa/movement-jitter/bevy-run-camera-transaction-20260718.json`
> and `.png`; the optimized `bevy-97470d40cbe1b310` smoke independently sampled
> 551 frames and six center changes with the same zero-failure result. Rust
> 107/107 plus the presentation-pose, local-command latency, scene-motion, and
> movement-controller web suites passed. Crystal's intentional
> six-phase 100ms stepped cadence remains; this closes the extra one-frame
> shake, not that original cadence.

> 2026-07-18 fresh-native TrapHexagon/Belt closure: live r05 captured the
> post-Rust-fix Crystal client with ordinary NPC primary names Lime and
> secondary underscore lines White at `0 @ 332,275`, Day setting 2. It exposed
> two deterministic Web regressions: `viewport-sprite-overlay` received
> `translate: 0px 0px`, creating an auto-level stacking context below the GPU
> entity canvas, and CSS rendered the nearly opaque Belt overlay at opacity
> 0.5. Correctly positioned Magic/1397 TrapHexagon nodes were therefore hidden,
> while six transparent Belt slots were darkened. Effects now live under an
> untransformed pass-through parent with per-node camera translation, and the
> non-equivalent Belt overlay is transparent. Final live r16 improves from the
> r05 15.0%/14.8% full/world changed ratios to 7.1%/6.0%, with world similarity
> 91.4%, world MAE 4.499, HUD UI 88.4%, Belt similarity 89.7% / MAE 10.765,
> chat 82.1%, and MiniMap 87.2%. The capture now fails if locked effects do not
> alter pixels: r16 sees 28 visible nodes and 57,282 changed pixels; forced
> WebGL2 r09 sees 55,462, both with 0 critical errors and 0 404s. Long native
> raw paths are read as Buffers, proven by r15 at 271 characters. The native
> top-left is still contaminated by the Codex Computer Use status bubble, so
> final clean pixel/human acceptance remains open.

> 2026-07-18 deterministic r03/r04 closure: the Web-only same-scene harness now
> completes Edge 150 CDP `Runtime.enable` through Next's compiled `ws`. r04 at
> Bichon `0.map @ 332,275`, paired Day setting 2, has 0 critical console errors,
> 0 404s, 100% runtime/layout/entity/pixel automated gates, a 100% weighted
> Candidate trend score, and a 93-100% estimated human band. This is not final
> acceptance: thresholded pixels still differ by 10% full-window and 9% world;
> chat is 82%, HUD UI 87%, and MiniMap 87%. The only r03 P0 was an Edge
> extension message-port closure, now narrowly ignored without masking real
> errors. Nameplate diagnostics confirm ordinary NPC primary lines Lime and
> secondary underscore lines White. Because the fixed r01 native frame was
> captured before the Rust duplicate White ObjectNpc path was removed, the next
> valid color comparison requires a fresh native/Web pair.

> 2026-07-14 full-pack/low-tier closure: all 1,440 Crystal libraries now have a
> deterministic, resumable, hash-verified offline conversion into 1,440 lazy
> manifest shards and 4,446 unique immutable PNG pages. All 2,143,132 frame
> slots are classified as 1,869,869 packed frames or 273,263 no-draw frames;
> the verified content hash is
> `f71b89aa38504c6c127b937043d4af6ecd26d9dd1a2b9ed3b91100e6a1f0052e`.
> Entity rendering prefers full-pack shards and retains the legacy path as a
> rollback. Bevy and raw WebGL2 caches now evict by entry and decoded-byte LRU,
> preserve active pages, release browser image references, and reject pages
> larger than the GPU limit. Forced-low WebGL2 Bichon QA held 13 pages and
> 1,598 rects in 58,379,430 bytes, passed 28/28 movement/render assertions, and
> used no DOM entity fallback. Low-tier prewarm fell to 403/403 successful
> requests with background warming off; cold transfer was 18,993,684 bytes and
> warm transfer 600 bytes, with 69,027,432 CacheStorage bytes. Evidence:
> `docs/generated/assets/crystal-full-pack-coverage.generated.json` and
> `docs/generated/player-qa/full-asset-pack-low-tier/`. This closes source-pack
> completeness and desktop low-tier automation, not all visual parity: maps
> remain regional, HUD/audio/effect-specific paths remain dedicated, and Brazil
> still needs CDN plus physical 2/4 GiB Android throttled-network soak.

> 2026-07-13 deterministic Crystal/Web temporal acceptance closure: one
> fail-closed scenario now records the native client and Web at Bichon
> `0.map @ 332,275`, aligns the same four left-walk actions, validates exact
> 1024x768 geometry, and emits bounded overlay/heatmap evidence. Two defects
> were coupled: the pose selector compared an ACK-advanced requested entity
> center with the still-rendered map center, and Rust pruned inactive standalone
> animation images while the Web upload set remembered them forever. The
> runtime now selects motion against the coherent applied map/entity center;
> each validated `map-render-synced` ACK also reconciles Web upload ownership
> with Rust's resident keys so a recurring frame is uploaded again. Runtime
> `bevy-90fb96239f221a47` passes the strict route on WebGPU with 4/4 pose events,
> 41ms maximum command-to-sink latency, and zero failed assertions; Bevy WebGL2
> passes the same movement gate at 4/4 and 42ms. Evidence is under
> `docs/generated/player-qa/movement-jitter/temporal-packs/bichon-332275-left4/`
> and `.../bichon-332275-left4-webgl2/`. The four native/Web pairs are aligned
> within 1-13ms, but their full-window changed-pixel ratios remain
> 75.0%-76.8%. That metric includes different population, lighting, HUD text,
> and effects, so it is not a movement failure and is not visual acceptance;
> those visible scene-composition gaps remain open.

> 2026-07-13 first-principles asset/render Candidate closure: the Web client now
> derives its runtime semantics from the complete Crystal source tree instead
> of hand-maintained frame guesses. The deterministic snapshot parses all
> 1,440 libraries (7,638,253,548 source bytes and 2,143,132 frame slots),
> including 703 non-empty v3 FrameSets and 3,643 actions with start/count/skip,
> interval, reverse, blend, and secondary-effect tracks. Player/NPC/monster
> presentation consumes those actions in production; player Spell uses the
> dedicated Crystal frame range at 296, and the packet-backed scene-effect
> queue resolves 62 spells, 11 object effects, two map effects, 35 explicit
> SpellEffect mappings, directional ranges, masks, offsets, light, and blend.
> Packed map pages now load directly in Bevy through `AssetServer` URLs, retain
> the previous complete frame until all replacement pages are ready, and use an
> exact generation/revision ACK before JS releases old image ownership. Unified
> atlas residency is ref-counted and bounded by decoded byte budgets, while the
> immutable CAS release/channel layout publishes assets before the mutable
> channel pointer. Full offline release verification passes with 38,846 assets,
> map renderability 191,938/192,391 (99.76%, with all references accounted for),
> minimaps 227/226 with none missing, SoundList 450/450 backed by 320/320 distinct
> wav files, and headline render coverage 99.88%. WebGPU and WebGL2 map smokes
> both report no failed assertions or critical console errors; evidence:
> `docs/generated/assets/crystal-source-snapshot.generated.json`,
> `docs/generated/assets/latest-asset-coverage-summary.json`,
> `docs/generated/player-qa/bevy-runtime-backends/bevy-runtime-backends-asset-pipeline-final-20260713.json`,
> `docs/generated/player-qa/bevy-map-standalone/bevy-map-standalone-webgpu-20260713001211-8821c193-report.json`,
> and `docs/generated/player-qa/bevy-map-standalone/bevy-map-standalone-webgl2-20260713001238-3c4011f6-report.json`.
> The final PNGs are fully opaque and have matching cross-screen RGBA samples;
> a viewer-only black surround was not present in the image bytes. This
> supersedes the older phase-1 note below
> that called FrameSet/runtime consumption open. Remaining work is final human
> Crystal-vs-Web lighting, density, and feel acceptance, not another asset
> architecture rewrite.

> 2026-07-13 map-render pipeline closure: the Bichon black rectangles were not
> missing map data. Raw packed atlases bypassed Crystal black-key conversion,
> including floor-sized frames stored in object libraries, while per-cell
> middle/front additive flags were discarded. Object-like frames now use the
> decoded standalone path, Mir3 `Dungeonsc` is covered, scene/cache and packaged
> starter blueprints carry explicit `normal|additive`, and the Bevy additive
> shader preserves Crystal's RGB equation without writing opaque black alpha to
> the transparent browser canvas. Floor layers now occupy a bounded band below
> objects/entities instead of using offsets that could overdraw buildings.
> At BichonProvince `0.map @ 320,43`, the same compressed screenshot crop went
> from 345 pure-black pixels before the fix to zero on both WebGPU and WebGL2;
> DOM map fallback and browser console errors were also zero. Evidence:
> `docs/generated/player-qa/map-rendering/bichon-320-43-map-pipeline-20260713.json`
> and matching final screenshots. Runtime 101/101, full frontend logic,
> TypeScript, map routing tests, and both release WASM builds pass. Remaining
> map-pipeline risks are GPU-ready ownership ACK precision, bounded additive
> material residency, and final Crystal lighting/effect visual acceptance.

> 2026-07-13 monster lock/chase acceptance: clicking a live monster now enters
> a persistent target-combat state instead of immediately sending one attack.
> The client reuses the authoritative movement intent/ACK pipeline, refreshes
> the adjacent destination when the monster moves, waits for the final accepted
> movement action to settle, then starts Crystal-local melee and sends only an
> in-range attack. The same lock continues at the local attack cadence until the
> target dies/disappears, selection changes, the map/session ends, or the player
> issues manual movement. Browser verification chased Royal Archer from player
> `310,51` to `320,43`, first observed `.attacking` at 2.3s with the target still
> selected and `Attack · 1 tiles`, then observed zero attacking samples for
> 3.5s after a manual ground click. Evidence:
> `docs/generated/player-qa/combat/web-monster-lock-chase-20260713.{json,png}`.
> `test:frontend-logic`, focused target-combat tests, and TypeScript pass. This
> closes target engagement flow only; transparent sprite hit interception and
> the separately reported map-rendering defects remain open.

> 2026-07-13 local melee fix and automated visual acceptance: pre-fix, an
> adjacent Space attack reduced Deer HP `11/25 -> 4/25` but the self
> `.attacking` class timed out at 900ms. Diagnostics proved the shared Zone echo
> used observer id `50001`, while the owner `SelfPlayer` uses personal id `1000`.
> Crystal queues local `Attack1` before sending `C.Attack` and ignores its own
> `S.ObjectAttack`; Web now mirrors that sequence for a live adjacent target.
> The rAF world flush remains packet-coalesced but is no longer a low-priority
> React transition, so 600ms combat windows cannot be starved. Post-fix the
> class appeared in 123ms, the screenshot captured a visible swing, and it
> detached normally after the action. Evidence:
> `docs/generated/player-qa/combat/web-local-melee-attack-20260713.{json,png}`.
> Full frontend logic and TypeScript pass. Stable all-direction/action atlas
> membership, Bevy cached-layout refresh, and combat-over-movement pose priority
> remain green. Separate open gap: alpha-transparent CherryTree sprite bounds
> can intercept pointer input intended for nearby entities.

Purpose: track frontend/client visual, interaction, and human-feel gaps separately from backend/server parity.

Status values:

- `[ ]` open
- `[~]` active
- `[x]` fixed and verified
- `[a]` accepted difference

## Current Automated Evidence

- 2026-07-13 compact-window movement flicker investigation: 264 pre-fix A/B
  frames across Bevy local-pose on/off and 820/1024 viewports produced zero
  scene blackouts and zero atomic-pose warnings. Entity/layer changes matched
  AOI membership rather than one-frame removals, so the defect was not a
  Gateway packet, entity-atlas failure, or DOM/Bevy ownership toggle. Crystal
  source confirms its 100ms movement phase is correct, while its integer/even
  `OffSetMove` is drawn directly into the selected backbuffer. Web instead had
  a 1024x768 canvas transformed to `820.02x615.01` at `top=102.49`, causing
  whole-scene fractional resampling. The responsive stage now derives an exact
  integer 4:3 rectangle and integer origin; the 820 regression is exactly
  `820x615` at `(0,103)` over 93 frames with zero blackout, pose, console, or
  404 warnings. Evidence: `docs/generated/player-qa/flicker-ab/current-820.json`,
  `current-1024.json`, `no-local-820.json`, and `aligned-820.json`. True pixel
  1:1 still requires a 1024x768-or-larger browser content viewport; compact
  presentation necessarily remains downsampled.

- 2026-07-12 canonical movement-presentation closure: WebGPU report
  `docs/generated/player-qa/movement-jitter/movement-mounted-scene-transaction-full-phases-webgpu-20260712-r12.json`
  passes 33/33 and WebGL2 report
  `docs/generated/player-qa/movement-jitter/movement-mounted-scene-transaction-full-phases-webgl2-20260712-r16.json`
  passes 33/33 on `bevy-bd9004a17f2873ea`. Both issue exactly one Walk and one
  mounted Run through keyboard, controller, WebSocket, shared Zone, and Bevy.
  They capture every Crystal Walk phase `0..7` at effective map offsets
  `-6,-12,-18,-24,-30,-36,-42,-48px` and Run phase `0..5` at
  `-24,-48,-72,-96,-120,-144px`, while the self sprite remains pinned.
  Map/entity centers never split, no synthetic non-endpoint logical centers
  appear, shadow command/ACK mismatches are zero, post-warmup pose provenance
  errors are zero, and phase, rollback, queue, console, and network warnings
  are zero. WebGPU ACKs are 2/6ms; WebGL2 ACKs are 7/2ms.
- The final causes were architectural rather than browser throughput: page
  logical position and Bevy both interpolated the same action; map and entity
  producers could expose half a scene transaction; one rejected pose changed
  immediately to a TypeScript clock; movement shadow hard-coded a Run target;
  and phase 0 advanced at the nearest global pulse, sometimes lasting only
  about 20ms. Bevy is now the sole local interpolation owner, scene provenance
  commits atomically, fallback uses a 250ms last-good-pose watchdog, shadow uses
  the explicit command target, and local phase cadence is anchored to
  `started + 100ms` with no catch-up. This closes the automated movement core,
  not overall visual parity: the screenshots still show lighting/effect and
  extra demo-population differences that remain open below.
- Unattended movement QA no longer risks attaching to a stale Chrome because a
  PID-derived debug port collided. Default capture lets Chrome allocate an
  ephemeral port, accepts only the `DevToolsActivePort` from that run's profile,
  and cleans up failed launches; explicit occupied ports are rejected before
  spawn. `movement-mounted-autocdp-cleanup-webgpu-20260712-r19.json` was
  produced without `--debugPort`, passes the same 33/33 movement gate, and
  leaves zero new Chrome profiles after completion.

- 2026-07-12 mounted movement acceptance candidate:
  `docs/generated/player-qa/movement-jitter/movement-mounted-walk8-run3-webgpu-20260712-r6.json`
  and matching `.png` exercise real keyboard input after granting, equipping,
  and using Crystal `RedTiger`. Exactly two commands are sent. Walk advances one
  tile in eight 100ms phases; Run advances three tiles in six phases. ACKs are
  18/22ms, final delta is `(4,0)`, local Pose coverage is 2/2 with a 26ms maximum
  sink latency, all 27 assertions pass, and pose atomicity/rollback/direction/
  queue/console/404 warnings are zero. Runtime is
  `bevy-78d40eb80133609c`; dual WebGPU/WebGL2 smoke report is
  `docs/generated/player-qa/bevy-runtime-backends/bevy-runtime-backends-phasecount-pose-final-20260712.json`.
- The mounted eighth-phase blackout was not GPU throughput. Rust emitted frame
  indexes 6 and 7 correctly, but the TypeScript Pose parser still rejected any
  `frameIndex > 5`. Pose motion now carries `phaseCount`, defaults legacy frames
  to 6, accepts Crystal's maximum 8, and rejects indexes outside that contract.
- 2026-07-12 final normal-port movement presentation pass: strict Release
  keyboard capture
  `docs/generated/player-qa/movement-jitter/movement-zone-owned-cadence-final-release-keyboard-20260712.json`
  exercises the actual input/controller/Bevy pose path rather than injecting
  raw packets. Walk and Run ACK in 23/6ms, both commands reach the pose sink in
  at most 12ms, final movement is exactly `(3,0)`, and logical rollback,
  direction lag, stale prediction, queue latency, camera stair-step, pose-frame
  atomicity, console, and 404 warnings are all zero. Its matching `.png` is a
  complete scene-ready WebGPU frame. Raw `packetSequence` captures remain
  protocol-only evidence and must not be interpreted as local-pose coverage.
- 2026-07-12 Zone-owned cadence/live-observer pass: realtime owner and AOI
  `UserLocation`/player appearance/removal/Turn/Walk/Run packets now reach a
  bounded token-fenced socket channel without waiting for React activity or an
  observer's private Session Tick. The Zone owner also advances one global
  300ms cadence, while personal ticks no longer multiply shared-world time.
  Strict Release evidence
  `docs/generated/player-qa/two-client-zone/two-client-zone-zone-owned-cadence-tick5000-release-20260712.json`
  holds personal Tick at 5000ms and sends no observer pulse: movement arrives
  in 12ms, both clients retain 16 entities, Bevy observes the remote packet and
  drives 29 packed offsets, with zero decode errors, queue drops, console
  errors, or 404s. The QA bridge now exposes live `worldRef` map/entities/tick
  getters so background-page rAF throttling cannot create stale automation
  state; StartGame/transfer snapshot timestamps and foreground screenshot
  readiness are also gated. Scene-ready screenshots are the matching `-a.png`
  and `-b.png` files beside the report.
- 2026-07-12 bounded Zone-ingress pass: normal Web movement no longer waits for
  a blocked private Session tick. The capacity-256 reader sends authenticated
  Walk/Run/Turn through a capacity-64 per-Zone actor while preserving serial
  action order, owner fencing, event publication, and save-transform sync.
  Release expired-run evidence
  `docs/generated/player-qa/movement-jitter/movement-protocol-expired-run-degrades-zone-ingress-release-keepalive-snapshot-ready-20260712.json`
  records ACKs at 15/14ms, one degradation, zero corrections, and `(2,0)`.
  Strict keyboard evidence
  `docs/generated/player-qa/movement-jitter/movement-normal-walk-run-chain-zone-ingress-release-keepalive-snapshot-ready-rerun-20260712.json`
  records ACKs at 17/21ms, pose/sink maxima at 23/24ms, zero failed assertions,
  and `(3,0)`. Event-observed evidence records 11/2ms and exactly one Walk plus
  one Run event. The harness waits for a new post-transfer world snapshot and
  fails a stuck CDP command after 15s instead of hanging indefinitely.
  Remote-observer push independence and one global Zone cadence are closed by
  the evidence above. Remaining feel gaps are mounted eight-frame motion, true
  three-cell sprint, lighting/effects, and final human side-by-side acceptance.
- 2026-07-12 movement degradation pass: the early page ACK path and controller
  reconciliation now use one `classifyMovementAckOutcome` decision. A requested
  Run acknowledged at its first cell is a confirmed Crystal-style degradation,
  not a correction that clears animation or arms the 400ms correction lock.
  Release raw protocol evidence
  `docs/generated/player-qa/movement-jitter/movement-protocol-expired-run-degrades-release-202607120745.json`
  records ACKs at 16/99ms, `degradedRunCount=1`, `correctionCount=0`, and final
  delta `(2,0)`. Normal UI Walk -> Run evidence
  `docs/generated/player-qa/movement-jitter/movement-normal-walk-run-chain-release-202607120750.json`
  records ACKs at 22/28ms, command-to-pose latency 17/1ms, zero degradation or
  correction, and final delta `(3,0)`. `npm.cmd run test:frontend-logic` is
  green. Bevy intentionally retains the TypeScript fallback for a degraded path
  until phase-preserving retargeting exists; taking over the wrong two-cell
  segment would be less faithful than that bounded fallback.
- 2026-07-12 default shared-clock and additive-world pass: the normal URL now
  enables guarded Bevy local self/camera ownership plus synchronous pose commit;
  the tested rollback is `?bevyLocalMotion=0&bevyPoseCommit=0`. A single
  Crystal-compatible 100ms pulse advances all six movement phases and does not
  freeze on delayed ACKs. Default continuous evidence
  `docs/generated/player-qa/movement-jitter/movement-default-shared-clock-continuous-202607120610.json`
  sent one click as three walks at 601/601ms and kept command-to-pose latency at
  10ms maximum. Committed keyboard evidence
  `docs/generated/player-qa/movement-jitter/movement-default-shared-clock-keyboard-committed-ref-202607120617.json`
  matched 4/4 commands, returned to `328,275`, and stayed within 15ms with zero
  long tasks, interaction pollution, warnings, errors, or 404s. Native/Web
  action-aligned evidence
  `docs/generated/player-qa/movement-jitter/temporal-crystal-native-vs-web-default-shared-clock-horizontal-20260712-001.md`
  measured the same 2701ms four-action span and 24 active Web frame pairs,
  matching four commands times six movement phases. Its full-window pixel
  ratio remains confounded by different world objects, ambient effects, HUD,
  browser chrome, and capture geometry; it is not an actor-isolated movement
  score. Explicit rollback evidence
  `docs/generated/player-qa/movement-jitter/movement-explicit-legacy-rollback-202607120623.json`
  proves both ownership flags inactive, 2/2 command and ACK matches, and exact
  coordinate return. The final 25 additive world sprites now render through a
  Bevy `SrcAlpha + One` material; WebGPU report
  `docs/generated/player-qa/bevy-map-standalone/bevy-map-standalone-webgpu-20260711213830-dee09cfc-report.json`
  and the matching WebGL2 smoke both report zero DOM world sprites and zero
  image/network failures. Runtime `bevy-630a77b3535f95bd` passes 94/94 Rust
  tests plus dual-backend report
  `docs/generated/player-qa/bevy-runtime-backends/bevy-runtime-backends-default-shared-clock-202607120620.json`.
  Remaining frontend gates are real correction/degraded-run capture, mounted
  eight-frame and sprint cases, scene population/ambient/light parity, and
  combat-effect polish.
- 2026-07-10 release early-pose and incremental-map pass: a clean local command
  can now own the unified Bevy pose immediately, without waiting for React to
  publish a delayed TypeScript motion window. Map and entity producers share one
  exact render center, viewport entities are rebased atomically, and correction,
  degraded-run, target-mismatch, and path-mismatch cases still fall back to the
  TypeScript path. Pose commit remains default-off behind `?bevyPoseCommit=1` /
  `mir2-bevy-pose-commit`, alongside the existing default-off local-motion flag.
  The normal `npm run dev` path now builds release WASM; the explicit diagnostic
  alternative is `npm run dev:debug-runtime`. Producer semantic deduplication and
  retained Rust map entities reduced the four-step route from revision `687 ->
  999` (about 70 revisions/second, 53 sampled states) to `13 -> 21` (five sampled
  states: initial plus four real centers). Existing tiles now update only their
  transform, image bindings change only on image revision, and the runtime no
  longer clones the roughly 202 KB draw list after every apply. Strict WebGPU
  evidence
  `docs/generated/player-qa/bevy-movement-shadow/bevy-movement-shadow-webgpu-20260710220403-44ba1f45-report.json`
  is fully green at exact final tile `328,275`: all 4/4 commands reached an
  accepted `localCommand` sink in `14/18/32/16ms` (32ms maximum under the 75ms
  gate), with zero drops, provenance failures, visual jumps, console errors, or
  non-favicon 404s. Default-off compatibility evidence
  `docs/generated/player-qa/bevy-movement-shadow/bevy-movement-shadow-webgpu-20260710221024-ce1066ce-report.json`
  is also green. Runtime `bevy-9ce93936c0841d7e` passes 86/86 Rust tests,
  TypeScript, scene/controller/pose/latency tests, and dual-backend report
  `docs/generated/player-qa/bevy-runtime-backends/bevy-runtime-backends-20260710221430.json`.
  Remaining acceptance gaps are an exact native correction/degraded-run temporal
  comparison and the longer-term world-space/chunk map model; the rollback flags
  must stay available until those gates pass.
- 2026-07-10 guarded Bevy local-motion presentation pass: copies of normalized
  self movement commands and authoritative ACKs now feed a bounded Rust
  `PreUpdate` resource. It can own the packed self sprite, Bevy camera, and DOM
  overlays through the existing unified pose buffer, but remains
  presentation-only: shared Zone still owns acceptance, correction, collision,
  occupancy, cooldown, AOI, and persisted transforms. Takeover is default-off
  behind `?bevyLocalMotion=1` / `mir2-bevy-local-motion`; disabling it preserves
  the previous TypeScript path. An object + target + from/to path handshake
  prevents a degraded run or visually rebased TS window from attaching the
  wrong Bevy segment. Corrections clear the segment, path/target mismatch falls
  back to TS, and a completed matched segment settles both camera and self at
  exact zero without reconnecting a delayed window. Runtime
  `bevy-e50cfdd1e6c8d229` passes Rust 83/83, pose parser 6/6, movement bridge
  9/9, TypeScript, release build, and validated WebGPU/WebGL2 packages. Final
  backend probe
  `docs/generated/player-qa/bevy-runtime-backends/bevy-runtime-backends-20260710173210.json`
  is fully green and explicitly proves mismatched-path `selfWindow` fallback,
  matched-path `localCommand` ownership, disable isolation, package fetches,
  raw WebGL2 rendering, and zero critical console errors. Real WebGPU A/B routes
  are both `ok=true`: default-off report
  `docs/generated/player-qa/bevy-movement-shadow/bevy-movement-shadow-webgpu-20260710173245-17db8e6b-report.json`
  records 76/76 exact local geometry samples, while forced-on report
  `docs/generated/player-qa/bevy-movement-shadow/bevy-movement-shadow-webgpu-20260710173356-7b3abddd-report.json`
  also records 76/76, final self/camera source `localCommand`, 4/4 command matches,
  4/4 ACK matches, 0 visual jumps, 0 queue/decode drops, 0 critical errors, and
  0 non-favicon 404s. The final map regression
  `docs/generated/player-qa/bevy-map-standalone/bevy-map-standalone-webgpu-20260710173500-ca321fe7-report.json`
  also remains green with 109 standalone draws and 108/108 decodes. Remaining
  acceptance gap: command-timestamp presentation differs from the delayed TS
  window by up to 32px / 326ms in this route. Keep takeover default-off until an
  exact native Crystal vs Web A/B frame sequence proves that earlier phase is
  closer to native and correction/degraded-run routes remain visually clean.
- 2026-07-10 unified Bevy presentation-pose pass: packed sprite transforms,
  self-camera translation, and residual DOM nameplates/HP/chat now consume the
  same per-frame Rust pose buffer. The packed wire marks `isSelf`; Bevy computes
  one camera screen pose at frame start, derives the self sprite as its exact
  inverse, records the actual selected remote/fallback offsets, and publishes a
  versioned 256-entry bounded snapshot. DOM reads it at rAF frequency and falls
  back to the previous TypeScript curve on missing, malformed, stale (>250ms),
  disabled, or unsupported runtime data. `?bevyPresentationPose=0` disables only
  the DOM bridge and cannot change Bevy rendering. The first real route exposed
  a genuine dual-window race as two 20/22px self-label jumps; centralizing the
  self pose removed both rather than weakening the test. Runtime
  `bevy-8a40d0bdcf0dc14a` passes Rust 72/72, pose-parser 5/5, movement-bridge
  9/9, TypeScript, and dual-backend release/self-check gates. Chrome/WASM report
  `docs/generated/player-qa/bevy-runtime-backends/bevy-runtime-backends-unified-pose-20260710.json`
  is fully green in default/forced WebGPU and forced WebGL2, including remote
  packet `-24px`, camera `+24px`, source tags, bridge disable isolation, package
  fetches, and console gates. Real keyboard-route evidence
  `docs/generated/player-qa/bevy-movement-shadow/bevy-movement-shadow-webgpu-20260710163125-1a4aff1b-report.json`
  is `ok=true`: 0 visual jumps, 4/4 command matches, 4/4 ACK matches, 1219 Bevy
  pose samples vs 4 startup fallbacks, 38908 entity-pose hits, 0 pose overflows,
  0 critical errors, and 0 non-favicon 404s. Final map regression
  `docs/generated/player-qa/bevy-map-standalone/bevy-map-standalone-webgpu-20260710162936-ca18422e-report.json`
  remains green with 109 standalone draws, 108/108 decoded images, and only 25
  DOM sprites, all additive. Next gap: migrate local self prediction and ACK
  reconciliation into a guarded Bevy presentation source while shared Zone
  remains authoritative for acceptance, correction, collision, cooldown, AOI,
  and persisted transforms.
- 2026-07-10 Bevy packet-driven remote-motion presentation pass: normalized
  `ObjectWalk` / `ObjectRun` / `ObjectTurn` / remove events now feed a bounded,
  presentation-only Bevy resource during `PreUpdate`, without changing input,
  collision, cooldown, AOI, reconciliation, persistence, or shared-Zone
  authority. Walk/run segments use Crystal's 600ms stepped cadence, connected
  segments continue from the currently displayed fractional pose, stale events
  are ignored, large discontinuities snap, and remove/disable clears state.
  Packed sprites consume the Rust offset only when the packet target matches
  the latest packed entity grid target; otherwise the existing TypeScript
  motion window remains the safe fallback. The path is default-on when packed
  Bevy entities are active and can be disabled with `?bevyRemoteMotion=0`.
  Runtime `bevy-63449641a633efc2` passes all 67 Rust runtime tests, including 13
  focused remote-presentation tests, and the TypeScript bridge passes 9/9.
  Real Chrome + WASM evidence
  `docs/generated/player-qa/bevy-runtime-backends/bevy-runtime-backends-remote-motion-probe-20260710.json`
  is `ok=true`: default WebGPU, forced WebGPU, and forced WebGL2 each proved the
  target-mismatch fallback, then matched-target Bevy offset takeover, then
  disable-and-clear, with zero decode/event drops and no critical console
  errors. Current map and movement regressions remain green at
  `docs/generated/player-qa/bevy-map-standalone/bevy-map-standalone-webgpu-20260710162936-ca18422e-report.json`
  and
  `docs/generated/player-qa/bevy-movement-shadow/bevy-movement-shadow-webgpu-20260710154640-c847d5b3-report.json`.
  This proves renderer ownership with synthetic packet injection, not real
  shared-Zone transport: repeated two-client runs exposed a native Gateway
  multi-session/reconnect crash (`0xc0000005` / `0xc0000374`), recorded at
  `docs/generated/player-qa/two-client-zone/two-client-zone-native-crash-20260710.json`.
  The unified sprite/camera/DOM pose work named here is complete in the pass
  above; local self prediction/reconciliation is the next guarded migration.
- 2026-07-10 Bevy movement shadow-ECS pass: the production input, WebSocket,
  and authoritative shared-Zone paths are unchanged, but every accepted local
  walk/run/turn decision and its `UserLocation` ACK now mirrors into an
  observation-only Bevy resource on a 100ms `FixedUpdate`. Rust independently
  derives the destination from source/direction/mode, correlates ACKs through a
  bounded FIFO, treats one-tile run landings as explicit degradation, requires
  direction parity for turns, and records bounded remote motion segments.
  The bridge cannot throw into production movement; pending event JSON is capped
  at 256, pending commands at 16, remote segments at 256, and ObjectRemove/Hide
  evicts remote state. Focused tests pass Rust 15/15 and TypeScript 9/9.
  Isolated WebGPU evidence
  `docs/generated/player-qa/bevy-movement-shadow/bevy-movement-shadow-webgpu-20260710154640-c847d5b3-report.json`
  is `ok=true`: a four-step Right/Right/Left/Left route produced 4/4 command
  matches and 4/4 ACK matches, 0 command/ACK mismatches, 0 queue or command
  drops, 0 pending commands, 0 bridge errors, 0 decode errors, 0 critical
  console errors, and 0 non-favicon 404s; random credentials, Gateway, Chrome,
  and temporary files were cleaned automatically. Screenshot:
  `docs/generated/player-qa/bevy-movement-shadow/bevy-movement-shadow-webgpu-20260710154640-c847d5b3.png`.
  Runtime `bevy-63449641a633efc2` passes
  `docs/generated/player-qa/bevy-runtime-backends/bevy-runtime-backends-remote-motion-probe-20260710.json`
  with the movement-shadow API present in default/forced WebGPU and forced
  WebGL2 packages. This is a shadow diagnostic milestone, not production motion
  ownership. The remote presentation step named here is complete in the pass
  above; self/camera and the DOM overlay pose bridge remain open.
- 2026-07-10 Bevy map standalone-texture and ownership-handoff pass: packed
  atlas misses with normal alpha blending now decode through the bounded
  standalone-tile cache and upload as Bevy `Image` assets; additive Crystal
  glows deliberately remain in the DOM until the Bevy material path supports
  the same blend equation. Runtime readiness is now independent from status
  telemetry such as `scene-ready` / `map-render-synced`, so those events no
  longer stop the 33ms world-snapshot emitter or disable the map renderer.
  DOM/WebGL2 ownership stays live until Rust publishes a complete
  `map-render-synced` acknowledgement for every required atlas image, and
  standalone sprites hand off only after the same acknowledgement. Failed
  atlas decodes are removed from the promise cache so transient failures can
  retry. Isolated WebGPU evidence
  `docs/generated/player-qa/bevy-map-standalone/bevy-map-standalone-webgpu-20260710162936-ca18422e-report.json`
  is `ok=true`: map `0 @ 324,41`, 421 atlas tiles, 109 standalone draws / 108
  decoded standalone images, 7 atlas pages / 115 total images, 0 standalone
  failures, 0 map 404s, 0 critical console errors, and exactly 25 remaining
  DOM sprites, all 25 additive. Screenshot:
  `docs/generated/player-qa/bevy-map-standalone/bevy-map-standalone-webgpu-20260710162936-ca18422e.png`.
  Backend package evidence
  `docs/generated/player-qa/bevy-runtime-backends/bevy-runtime-backends-unified-pose-20260710.json`
  passes default/forced WebGPU, forced WebGL2, package-fetch, raw WebGL2 probe,
  and console gates; the current runtime is `bevy-8a40d0bdcf0dc14a`. Remaining map
  renderer gap: implement Crystal-compatible additive materials in Bevy, then
  remove the final 25-sprite DOM world-render fallback.
- 2026-07-10 native/Web movement temporal rerun: after the Crystal stepped
  motion pass, the automated comparison now has valid same-cadence window-frame
  evidence instead of the earlier black native capture. Native Crystal was
  relaunched from `E:\mir2\Crystal\Build\Client\Debug\Client.exe`, logged in as
  `cdx0708235326`, and captured
  `docs/generated/player-qa/movement-jitter/original-crystal-valid-step-route-20260710.json`
  with 90 JPEG frames, four real Computer Use clicks, average sample delta
  `50.12ms`, and no black-screen/device-lost frame set. Web was captured
  against the live `7111` Gateway with a fresh QA account and window-level
  frame capture:
  `docs/generated/player-qa/movement-jitter/web-crystal-window-fresh-step-route-20260710.json`
  (`ok=true`, 86 JPEG frames, average sample delta `50.11ms`, 3/3 walk ACKs,
  avg ACK `233ms`, max ACK `457ms`, 0 failed assertions, 0 entity-hit
  pollution, 0 critical console errors, 0 non-favicon 404s). The report
  `docs/generated/player-qa/movement-jitter/temporal-crystal-native-vs-web-window-20260710.md`
  records aggregate visual delta/sec Crystal `68.0367` vs Web `37.9166`
  (Web ratio `0.5573`). Interpretation: the Web movement pipeline is
  protocol-clean and sampled at native cadence, but the current moving scene
  still has only about 56% of native's per-second visual motion energy in this
  capture. Next frontend gap: use this evidence to tune residual camera/object
  draw/layer motion and then rerun an exact-route pack; do not judge the
  remaining feel gap from static screenshots alone.
- 2026-07-09 Crystal movement/render cadence pass: source audit confirmed the
  Web map/entity viewport origins already match Crystal's split anchors
  (`DrawFloor/DrawObjects` use tile-left origin `470`, while entity
  `DrawLocation` uses `480` at 1024x768), so this round deliberately did not
  "fix" the 10px floor/entity difference. The actual hand-feel mismatch was
  the movement offset curve: Web/Bevy used a free-running linear lerp while
  Crystal advances walking/running through 6 movement frames on the
  `GameScene.CanMove` 100ms cadence and truncates movement offsets to even
  pixels. Web `original-client-scene-motion.ts` and Bevy runtime
  `motion.rs` now share that stepped cadence for entity offsets, camera
  offsets, and fractional chained movement. Runtime packages were rebuilt as
  `bevy-e48cd43dadfddb17`. Verification passed focused Web
  `node scripts/test-scene-motion.mjs`, Rust
  `cargo fmt --check; cargo test --lib motion -- --nocapture` (24/24), and
  Bevy backend smoke
  `docs/generated/player-qa/bevy-runtime-backends/crystal-step-motion-runtime-20260709.json`
  with `ok=true`, package fetches healthy, default WebGPU selected, forced
  WebGL2 rendered, and 0 critical console errors. Remaining frontend gap:
  rerun same-route native/Web movement video capture to score temporal parity
  after this cadence change, then continue map-cell/object light tuning.
- 2026-07-09 Crystal/Web main-scene light render pass: Web now renders a
  Crystal-style scene light overlay for non-Day `lightSetting` values. Day and
  Normal keep the previous no-overlay path, while Dawn/Evening/Night mount
  `.viewport-crystal-light-overlay` between sprite rendering and nameplates so
  the world/actors darken but HUD, MiniMap, chat, and labels stay readable like
  Crystal's `DrawLights()` order. Evidence
  `docs/generated/player-qa/visual-parity/scene-light-render-20260709/`
  uses a temporary updated Gateway on `7311`, enters `demo` / `demo` as
  `Scout`, and records the clean screenshot
  `scene-light-render-clean-20260709.png` plus DOM state
  `overlayClass=viewport-crystal-light-overlay night`,
  `overlayLight=4`, `z-index=6`, `pointer-events=none`, `tutorialOpen=false`,
  and browser console errors `0`. The same pass now exports
  `OriginalMapCell.light` and renders viewport map-cell light nodes inside the
  overlay; API probe `map-light-export-probe-20260709.json` confirms map `0`
  samples with 127 / 127 / 25 / 26 light cells. A fresh map-light DOM screenshot
  was not captured because the real Crystal UTC light window rotated back to
  Day, correctly suppressing non-Day overlay rendering. Remaining frontend gap:
  recapture Night/Evening/Dawn map lights, tune intensity against native
  screenshots, and add object/equipment/effect light sources.
- 2026-07-09 Crystal/Web dynamic TimeOfDay/lightSetting pass: Web now receives
  the same dynamic light state that Crystal's server sends. Crystal source
  seeds `Envir.Now` from `DateTime.UtcNow` and maps `Now.Hour * 2 % 24` to
  Dawn/Day/Evening/Night; Simulation StartGame and `WorldSnapshot.lightSetting`
  use the same formula, and the browser applies `snapshot.lightSetting` plus
  exposes it through `window.__mir2Stage5.state.lightSetting`. Evidence
  `docs/generated/player-qa/visual-parity/light-setting-snapshot-20260709/`
  records direct WS `TimeOfDay.lights=4`, `worldSnapshot.lightSetting=4`, and
  browser state `lightSetting=4` with 0 critical console errors and 0
  non-favicon 404s. This closes light-state propagation only; the active
  frontend gap is still the main-scene Crystal ambience render for Night,
  Evening, and Dawn.
- 2026-07-09 Crystal/Web 335,266 evidence ladder: pack
  `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0060-minimap-source-panel-viewrect-native335266-clean/`
  is the latest clean rebuilt-gateway same-coordinate proof for account
  `cdx0708235326`: runtime/layout/entities `100%`, 0 network 404s, 0 critical
  console errors, MiniMap `86%`, HUD UI `86%`, and Web player `0 @ 335,266`
  with native-synced vitals/items/gold/belt. Follow-up packs
  `0061-chat-override-replace-native335266` and
  `0062-current-chat-colors-native335266` fixed capture-only chat behavior
  (`crystalVisibleChatLines` replaces startup logs; `[Mode]`, `[Pet]`, and
  `Now in Net` infer Crystal green/blue channels), but also confirmed native
  `LineMessage.txt` rotation/history makes chat pixels unstable unless the
  current native visible slots are controlled. Packs
  `0063-belt-quantity-ones-native335266` through
  `0065-belt-label-colors-native335266` fixed Web Belt quantity `1` visibility,
  black shortcut labels, and yellow belt counts; 4x crops verify the visual
  correction, while the remaining `hud-belt=78%` is mostly transparent-slot
  exposure of world/camera/light mismatch rather than missing Belt data.
  Validation: web `npm.cmd exec tsc -- --noEmit` passed. Next frontend gap:
  camera/viewport parity plus world light render/AOI/object-set alignment before
  using HUD-belt/chat pixels as acceptance gates.
- 2026-07-09 Crystal/Web MiniMap light-icon bootstrap pass (historical,
  superseded by dynamic light state above): this Web same-scene lane stopped
  bootstrapping the browser into Night for the current Crystal Day/Normal
  capture. Crystal maps `LightSetting.Day` and `LightSetting.Normal` to
  `Prguse/2093`, while Night maps to `2092`; Web was seeing the old fixed
  `TimeOfDay { lights: 4 }` path and rendered `2092`. Simulation StartGame
  bootstrap emitted `lights=2` for this proof, and evidence
  `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0057-minimap-light-day-bootstrap/`
  records Web `miniMapLight.originalSrc=/original-ui/Prguse/2093.png`, player
  `334,263`, 0 network 404s, 0 critical console errors, runtime/layout/entities
  `100%`, overall `98.4%`, pixel trend `95.7%`, and MiniMap moving from the
  0056 fair-coordinate proof `0.784` / meanAbsDelta `32.788` to `0.786` /
  `32.545`. Remaining MiniMap work is true raster/color/marker parity, not the
  time-of-day icon.
- 2026-07-09 Crystal/Web fair-coordinate evidence gate: the same-scene pack
  exposed that `qa.applyNativeState` could update vitals/items but leave the
  shared Zone authoritative transform at the previous coordinate, causing
  native/Web world and MiniMap comparisons to be one tile apart. The capture
  harness now verifies `mapFileName` plus `position.x/y` during
  `qa.applyNativeState`, and the Gateway shared-Zone path now syncs native-state
  transforms into Zone presence. Evidence
  `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0056-main-hud-fair-visible-coord/`
  records both Web `player` and `authoritativePlayer` at `334,263`, transfer
  mode `alreadyAtTarget`, 0 network 404s, 0 critical console errors,
  runtime/layout/entities `100%`, overall `99.5%`, pixel trend `98.6%`, world
  `85.8%`, HUD UI `86.8%`, chat `85.8%`, and MiniMap `78.4%`. Use 0056 as the
  current fair coordinate-lock proof before judging MiniMap/world deltas.
- 2026-07-09 Crystal/Web main-HUD content-y pass: Web keeps the
  `.main-hud-shell` anchored at `0,616` for layout parity, but shifts the
  inner `.main-hud` content down by `2px`. Pixel analysis of the 0050 and 0054
  crop pairs showed the main-HUD-only subregions (`hud-left`,
  `hud-right-controls`, `hud-right-status`, and `hud-bottom-center`) all had
  their best alignment with Web shifted down 2px, while independent Belt and
  Chat crops did not. Evidence
  `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0055-main-hud-content-y-offset/`
  records 0 network 404s, 0 critical console errors, runtime/layout/entities
  `100%`, and the HUD improvements from 0054: `hudRightControls` similarity
  `0.720` / meanAbsDelta `49.436` to `0.986` / `0.303`;
  `hudRightStatus` `0.734` / `42.642` to `0.824` / `14.189`; `hudUi`
  `0.782` / `34.113` to `0.856` / `15.453`; and `hudBottomCenter` `0.800` to
  `0.886`. Treat 0055 as a HUD proof, not a new fair overall baseline, because
  world/minimap/chat differed dynamically in this run (`overall=95.9%`,
  `chat=70.7%`, `world=77.3%`).
- 2026-07-09 Crystal/Web Belt overlay draw-order pass: Web now mirrors Crystal
  `InventoryDialog.BeltDialog` rendering order. Crystal hooks
  `BeltPanel_BeforeDraw`, and `MirControl.Draw()` calls `BeforeDrawControl()`
  before `DrawControl()`, so the `Index + 1` Belt overlay (`1933` horizontal,
  `1945` vertical) is drawn at `0.5F` opacity behind the main Belt frame
  (`1932` / `1944`). Web previously rendered the overlay after the base image,
  which darkened the Belt panel and item slots. Evidence
  `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0054-belt-overlay-draw-order/`
  uses the new auto-generated crop pairs and records `hudBelt` improving from
  the 0050 baseline similarity `0.765` / meanAbsDelta `48.963` to `0.791` /
  `38.920`; `hudUi` also moves from `0.778` / `35.215` to `0.782` / `34.113`.
  Treat 0054 as the Belt proof, not a new overall baseline, because native chat
  rotated during capture (`chat=75.9%`, overall `97.9%`).
- 2026-07-09 Crystal/Web same-scene crop automation: `capture-crystal-web-pack.mjs`
  now writes native/Web crop pairs for the same regions used by
  `report-crystal-visual-parity.mjs`: `world`, `hud-full`, `hud-left`,
  `hud-belt`, `hud-right-controls`, `hud-right-status`,
  `hud-bottom-center`, `minimap`, and `chat`. Evidence
  `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0053-auto-region-crops/`
  confirms 9 crop pairs are generated and recorded in the pack summary. Treat
  0053 as an evidence-tooling validation, not a new visual baseline: the native
  chat line rotated again, dropping chat to `67%` and overall to `96.9%`, while
  the right-status HUD metric returned to the 0050 baseline
  (`hudRightStatus=0.734`, meanAbsDelta `42.642`). The attempted 0051/0052
  GDI-outline HUD text experiments were diagnostic only and were not retained,
  because they did not improve the right-status similarity over 0050.
- 2026-07-09 Crystal/Web clean chat-slot baseline: Web capture now supports a
  `crystalVisibleChatLines` JSON override for same-scene evidence, allowing the
  harness to reproduce the native client's current four visible ChatDialog
  slots instead of comparing against a randomly rotated or scrolled
  `LineMessage.txt` state. Evidence
  `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0050-chat-visible-slots-current/`
  records Web visible chat lines
  `Online Players: 1 / Welcome to Crystal Mir 2 released by Suprcode. / Online Players: 1 / Online Players: 1`,
  0 network 404s, 0 critical console errors, runtime/layout/entities `100%`,
  overall `98.5%`, pixel trend `96%`, chat `83%`, HUD full `78%`, HUD UI
  `78%`, world `83%`, and MiniMap `80%`. The run also preserves the 0046
  weight-bar fix (`weightRatio=0.2258`, `fillWidth=16`,
  `originalSrc=/original-ui/Prguse/76.png`) and keeps `hudRightStatus=0.734`.
  Use 0050 as the latest fair automated visual baseline; 0047-0049 are retained
  as diagnostics for LineMessage rotation and line-slot control.
- 2026-07-09 Crystal/Web chat LineMessage capture-control diagnostic:
  `capture-crystal-web-pack.mjs` now accepts `--gatewayWs` and
  `--crystalLineMessage`, appending those query parameters to the Web base URL
  before the browser capture. This removes the brittle hand-built URL problem
  seen during 0046 retries and proves the Web startup chat can be seeded with
  the currently visible Crystal `LineMessage.txt` entry. Evidence
  `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0047-chat-line-message-sync/`
  records visible Web chat lines
  `Online Players: 1 / Make sure to follow JevLomcn on github for the latest Database releases. / Online Players: 1 / Online Players: 1`,
  keeps the 0046 weight-bar diagnostics (`weightRatio=0.2258`, `fillWidth=16`),
  and has 0 network 404s / 0 critical console errors. The chat crop still
  scores only `65%` because native Crystal leaves an empty/filtered line slot
  before the LineMessage while Web renders the seeded lines contiguously. Treat
  0047 as a chat `History` / `StartIndex` diagnostic, not a visual-score
  improvement; 0046 remains the current weight-bar proof and 0042 remains the
  cleaner fair overall score.
- 2026-07-09 Crystal/Web HUD weight-bar source-fill pass: Web now mirrors
  Crystal `MainDialogs.cs` for the main-HUD weight bar. Crystal sets
  `WeightBar.DrawImage = false` and draws only
  `(WeightBar.Size.Width - 2) * CurrentBagWeight / Stats[BagWeight]` pixels in
  `WeightBar_BeforeDraw`, choosing `Prguse/76` at <=50%, `UI_32bit/473` at
  <=75%, and `UI_32bit/472` above 75%. Web previously rendered `Prguse/76.png`
  as a full 76px bar for every weight state. Web now clips the source sprite
  to the Crystal fill width, records the DOM fill diagnostics, and exports the
  missing `UI_32bit/472.png` and `UI_32bit/473.png` resources. Evidence
  `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0046-weightbar-source-fill/`
  records `currentWeight=14`, `maxWeight=62`, `weightRatio=0.2258`,
  `fillWidth=16`, `originalSrc=/original-ui/Prguse/76.png`, 0 network 404s,
  0 critical console errors, runtime/layout/entities `100%`, and a measured
  right-status improvement from 0045's similarity `0.727` / meanAbsDelta
  `45.137` to `0.734` / `42.642`. Overall is `97%` because the chat crop is
  still dynamically mismatched (`71%`); use this pack for the weight-bar proof
  and keep 0042 as the cleaner fair overall score.
- 2026-07-09 Crystal/Web HUD right-button coordinate pass: Web now aligns the
  right-side main-HUD button coordinates with Crystal `MainDialogs.cs` for the
  1024px HUD. Crystal positions the buttons at `Size.Width - 105/55/119/96/73/50/27`
  (`919`, `969`, `905`, `928`, `951`, `974`, `997`), while Web had each button
  one pixel further left. The CSS button anchors now use the Crystal source
  coordinates. Evidence
  `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0045-hud-right-button-source-coords/`
  records 0 network 404s, 0 critical console errors, runtime/layout/entities
  `100%`, and a small right-controls improvement: `hudRightControls`
  similarity `0.715 -> 0.720`, meanAbsDelta `51.576 -> 49.436` compared with
  the cleaner 0042 baseline. Overall remains `97%` in 0045 because the chat
  crop is dynamically mismatched (`67%`), so use this pack for the right-button
  coordinate proof and keep 0042 as the cleaner overall visual score.
- 2026-07-09 Crystal/Web Belt label and HUD-subregion diagnostic pass: Web now
  mirrors Crystal `BeltDialog` shortcut-label layering. Crystal creates
  `Key[i]` as direct `BeltDialog` children at `(8 + i*35, 2)` for horizontal
  mode, while item cells sit at `(i*35 + 12, 3)`; labels therefore remain
  visible over occupied potion slots. Web previously nested labels inside each
  slot, adding an accidental 12px offset and letting potion buttons cover
  labels `1` and `2`. The labels are now rendered as direct belt children with
  Crystal parent coordinates and a higher z-index. The capture harness now
  records `labelRect`, and the visual report now emits clean HUD subregions
  (`hudLeft`, `hudBelt`, `hudRightControls`, `hudRightStatus`,
  `hudBottomCenter`) plus a `hudUi` aggregate so full-HUD world/edge pollution
  is visible instead of hidden. Evidence
  `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0044-belt-key-label-diagnostics/`
  records belt label rects `1 @ 238,620 26x14`, `2 @ 273,620 26x14`, 0 network
  404s, 0 critical console errors, runtime/layout/entities `100%`, and
  `hudUi=78%` with subregions `left=79%`, `belt=77%`, `rightControls=72%`,
  `rightStatus=73%`, `bottomCenter=80%`. The overall score is `97%` because
  the native/Web chat crop in this sample is dynamically mismatched (`67%`);
  use the screenshot crop/DOM label evidence for this Belt fix and keep the
  cleaner 0042 pack as the latest fair overall visual score.
- 2026-07-09 Crystal/Web MiniMap label/light/radar parity pass: Web now mirrors
  the source-level MiniMap label, light indicator, and radar-dot behavior used
  by Crystal's `MiniMapDialog`. Crystal sets
  `LocationLabel.Text = Functions.PointToString(...)`, whose format is
  `"{0}, {1}"`, so Web displays `335, 262`; the coordinate label now keeps the
  same `56x18` vertically centered box, and MiniMap labels use Arial like
  Crystal `MirLabel`. Missing `Prguse` light frames `2092`, `2094`, and `2095`
  were exported so the Web light indicator follows Crystal's `TimeOfDay`
  mapping. The radar overlay now draws Crystal-style 2x2 `RadarTexture` rects
  at `(x - 0.5, y - 0.5)`, skips dead entities, and preserves Crystal's
  white/player, green/NPC, red/other, and blue/owned-object color path where
  the Web state exposes ownership. Evidence
  `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0042-minimap-radar-dot-label-welcome/`
  records `miniMapLightOriginal=/original-ui/Prguse/2092.png`, 0 network 404s,
  0 critical console errors, overall `98%`, estimated human band `91-100%`,
  pixel trend `96%`, HUD `78%`, world `83%`, minimap `80%`, and chat `83%`.
  MiniMap meanAbsDelta moved slightly from the 0039 `29.718` to `29.535`;
  crop pairs and `*-diffx4.png` heatmaps are attached. Remaining minimap work
  is now true raster crop/color and source sampling parity, not coordinate,
  light-icon, label-box, or radar-dot semantics.
- 2026-07-09 Crystal/Web HUD text parity pass: Web now mirrors Crystal
  `MainDialogs.cs` for the main-HUD gold label
  (`GoldLabel.Text = GameScene.Gold.ToString("###,###,##0")`) and pins the
  main HUD to Crystal's default `Settings.FontName = "Arial"` instead of
  inheriting the page's Georgia serif. The HUD no longer renders raw `3457`;
  same-scene Web DOM state and right-HUD crops show `3,457`, matching native
  Crystal. Evidence
  `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0036-hud-font-arial-cleanline/`
  records HUD gold text `3,457`, weight `48`, space `38`, HP `51/51`, EXP
  `48.33%`, 0 network 404s, 0 critical console errors, overall `98%`,
  estimated human band `91-100%`, pixel trend `95%`, HUD `77%`, world `83%`,
  minimap `79%`, and chat `82%`. The 0034 gold-only pass scored HUD `78%`,
  so the font fix is kept as source-backed visual cleanup rather than a
  claimed score win. Remaining HUD work is true bottom-panel asset/layout
  drift, button/glow/text placement polish, and residual chat scrollbar/font
  pixels.
- 2026-07-09 Crystal/Web ChatDialog and HP orb parity pass: startup chat
  content/state now follows the native running Crystal client instead of
  showing Web debug/status pollution. Web seeds the same visible four-line
  window (`Online Players`, the current Crystal `LineMessage`, then two more
  `Online Players`) while keeping the backend `Welcome` chat in older history,
  supports a `?crystalLineMessage=...` capture override for Crystal's rotating
  `Envir/LineMessage.txt`, maps `ChatType.LineMessage` to a blue/white
  Crystal line, renders chat rows as AutoSize-width labels, and hides the empty
  input box like Crystal `ChatTextBox.Visible=false`. The low-level Warrior
  HP-only orb also no longer uses the two-resource 50px half-orb crop, so full
  HP renders as a complete red orb. Evidence
  `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0033-chat-and-hp-orb-clean/`
  captured `Online Players: 1 / Welcome to Crystal Mir 2 released by Suprcode.
  / Online Players: 1 / Online Players: 1` on both clients, kept HUD readouts
  at `HP 51/51`, `MP 32/32`, EXP `435/900`, gold `3457`, and weight `14/62`,
  and scored overall `98%`, estimated human band `91-100%`, pixel trend `96%`,
  HUD `78%`, and chat `83%` with 0 network 404s / 0 critical console errors.
  Remaining gaps are now HUD bottom-panel asset/layout drift, residual chat
  font/scrollbar pixels, world scene review (`83%`), and minimap crop/color
  (`79%`).
- 2026-07-09 Crystal/Web bottom-right HUD parity pass: the main HUD now follows
  Crystal `MainDialogs.cs` semantics for the two small right-side readouts.
  Web `WorldSnapshot.maxWeight` is sourced from Crystal player stats instead
  of the old fixed `100`, and the main HUD displays remaining bag weight
  (`maxWeight - currentWeight`) plus Crystal's 46-slot inventory free-space
  view, with the gold row visible below it. Evidence
  `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0027-hud-weight-diagnostics/`
  captured the same native-state character at `0 @ 335,262` with
  `currentWeight=14`, `maxWeight=62`, HUD `weight=48`, HUD `space=38`, and
  gold `3457`; the right-HUD crop now visually matches Crystal's
  `48 / 38 / 3,457` readout. The score remains overall `94%`, estimated human
  band `87-100%`, runtime/layout/entities `100%`, pixel trend `85%`, and 0
  network 404s / 0 critical console errors. Remaining HUD work is now broader
  asset/layout and chat-panel parity, not this bottom-right status semantic.
- 2026-07-09 Crystal/Web native-state, max-MP, and EXP-curve pass: same-scene capture can now
  seed Web from the native Crystal account state and apply the same live
  character snapshot through the token-gated QA-control path before scoring.
  Evidence
  `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0025-exp-debug/`
  captured `Cdx0708235326` on `BichonProvince` map `0 @ 335,262` with Web
  state aligned to native level `6`, `HP 51/51`, `MP 32/32`, EXP `435/900`
  (`48.33%`), gold `3457`, 6 inventory items, 2 belt items, and 8 equipment
  items. The latest score is overall `94%`, estimated human band `87-100%`,
  runtime/layout/entities `100%`, pixel trend `85%`, and 0 network 404s / 0
  critical console errors.
  This clears the previous P0 runtime hygiene issue from missing potion icons
  (`Items/398.png`, `Items/394.png`) and the post-snapshot `playerMaxMp`
  drop (`32` now remains visible in Web state after transfer). The upsert path
  now reads Crystal `ExpList.ini`, so level-6 max EXP is `900` instead of the
  old Web placeholder `100`. Remaining visible gaps are true frontend parity
  work rather than account-state pollution: P1 HUD assets/layout (`71%`,
  especially chat overlap and remaining bottom-panel asset drift), P2 world human
  review (`83%`), P2 minimap crop/color (`79%`), and P2 chat content/state
  (`62%`).
- 2026-07-09 Crystal/Web HUD-state diagnostic pass: same-scene evidence now
  includes both Web HUD/item DOM state and a read-only extraction of native
  Crystal `Server.MirADB` account state. Evidence
  `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0019-hud-state-diagnostics/`
  keeps runtime/layout/entities green at `100%`, overall `95%`, pixel trend
  `87%`, and 0 network 404s / 0 critical console errors, but explicitly marks
  the top P1 gap as dynamic state pollution rather than pure HUD art drift.
  Web is captured as `Cdx0708235326` level `1`, `HP 18/18`, `MP 14/?`, gold
  `0`, empty belt/inventory, and starter Web equipment; native Crystal
  account state for the same visible character is level `6`, HP `51`, MP `32`,
  gold `3457`, belt `(HP)DrugSmall` + `(MP)DrugSmall`, and equipment
  `EbonySword`, `BaseDress(M)`, `Candle`, `GoldNecklace`,
  `WornIronBracelet`, `CopperRing`, `OldCopperRing`, `OldLoafer`. The next HUD
  pass should therefore align capture character state before treating the
  remaining HUD/chat pixels as asset/layout defects.
- 2026-07-09 Crystal/Web same-account same-scene blend pass: Bevy map rendering now leaves
  Crystal additive glow sprites on the DOM fallback instead of folding them
  into normal-alpha atlas tiles. DOM blend sprites use cleaned
  `/generated/original-map-blend/...` frames with `mix-blend-mode: screen`;
  tall blue/white columns use opacity `1` plus
  `brightness(2.35) saturate(1.08)`, while compact Bichon torch glows use
  opacity `0.78` plus `brightness(2.25) saturate(0.72)`. The visual capture
  scripts now support `--createAccount` / `--characterName`, allowing Web
  evidence to use the same visible character name as the native Crystal
  client. Evidence
  `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0017-same-account-native335/`
  captured same-name Crystal/Web at the native client coordinate `0 @
  335,262`: overall visual score `97%`, estimated human band `90-100%`,
  runtime/layout/entities `100%`, pixel trend `92%`,
  `bevyMapRenderer.tileCount=400`, `domBlendSpriteCount=12`, and
  `network404Count=0` / `criticalConsoleErrorCount=0`. Remaining visible gaps
  from the same report are HUD state/assets (`77%`), world human review
  (`90%`), minimap crop/color (`80%`), chat panel state (`71%`), and mismatched
  HP/MP/equipment/belt/HUD state between native and Web captures.
- 2026-07-08 QA-control rerun: the browser automation can now use a safe local
  control wrapper instead of pretending production clients may send debug
  commands. `qaControl` is token-gated and production command safety remains on.
  Report
  `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-qacontrol2-20260708/report.md`
  passed incoming damage (`18 -> 0`) and death/revive (`0 -> 18`) on Rust
  `7111`. Active frontend gaps from the same evidence: server sent a
  `DamageIndicator` but DOM `.scene-damage-floater` stayed at peak `0`; the
  seeded pickup route failed to reach the Blue Potion tile; normal kill/XP/drop
  remain red; Monster `007` original-ui metadata still 404s. Automation gap:
  QA-control transfer/spawn needs explicit ack/settle handling because some
  packets landed late in the trace.
- 2026-07-08 Rust `7111` attack-trace rerun: incoming monster damage is now
  proven from the real Web client rather than inferred from backend-only tests.
  The updated harness records map/object ids, sent attack frames, melee
  approach, delayed server combat packets, and retry details for the first
  `StartGame` race. Evidence
  `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-survivalattacktrace5-20260708/report.md`
  reached melee with `ForestYeti` object `258949`, captured target
  `ObjectAttack`, `ObjectStruck`, and `DamageIndicator` packets, and saw player
  HP fall `18 -> 3`. Active frontend/client gaps remain: normal attack-kill,
  XP, loot, and death/revive feel still need a stable same-scene rerun; the
  current QA control lane is unreliable because `transferMap` reports sent but
  map/position stay unchanged, `event.spawn RakingCat0` yields no visible
  hostile, and death/revive can fail if a live hostile keeps attacking during
  the revive beat. Missing original sound/monster metadata 404s still affect
  feel.
- 2026-07-08 Rust `7111` pickup/death rerun: the drop-click route is now less
  overdriven by QA fallback movement, and latest targeted evidence
  `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-pickupwait5s-20260708/report.md`
  passes deterministic Blue Potion pickup plus death/revive on the real Rust
  gateway. This confirms the current pickup failure class is no longer the stale
  predicted/self-coordinate bug. Active gaps remain: hostile-retaliation proof
  still needs a stable adjacent attack packet sequence (`survivaltick` did not
  produce accepted player-damage evidence), real kill/XP/drop should be rerun
  with a normal combat window, and original sound/monster metadata 404s still
  affect feel.
- 2026-07-08 Rust `7111` pickup/death lane: Web action gating now uses
  `state.authoritativePlayer` from packet ACKs instead of predicted/render
  self, and the QA harness counts carried items across inventory plus belt.
  Evidence
  `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-authpickupseed7-20260708/report.md`
  passed deterministic Blue Potion pickup (`GainedItem x1`, carried `0 -> 1`)
  and death/revive via `@DIE` plus `townRevive` (`0 -> 18`, respawn
  `0:330,270`). This closes the specific frontend-side pickup misclassification
  and stale-coordinate action-gating gap. Active frontend/client gaps remain:
  combat kill/XP feedback still lacks green evidence in this seeded run,
  monster retaliation did not reduce HP, and missing original UI sound assets
  (`Sound/103.wav`, `Sound/144.wav`) still create runtime 404s.
- 2026-07-07 Rust-gateway combat/effect settled pass: damage feedback is no
  longer purely backend-blocked. `qa-combat-survival.mjs` now uses normal
  `walk` packet fallback when WebGL2 has no DOM tile hit layer, rotates melee
  approach tiles, and settles late CDP WS frames before scoring. `page.tsx`
  sends targeted combat-confirm ticks and renders `DamageIndicator` directly
  into the scene overlay. Evidence
  `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-floaterfix30s-20260707/report.md`
  connected to Rust `7111`, landed melee damage, saw target HP fall
  (`minPercent=95`), observed 4 server damage indicators, and passed the DOM
  `.scene-damage-floater` gate with peak 1. Active frontend/client gaps remain:
  kill/death animation, loot/XP feedback, and death/revive UI cannot be
  accepted until backend gameplay emits `ObjectDied`/XP/drop/dead-state
  evidence; missing original UI sound/monster metadata still causes run-time
  404s.
- 2026-07-07 Rust-gateway combat/effect anchor pass: the current combat gap is
  now a strong backend/effect integration blocker, not just a frontend feel
  suspicion. Hardened `qa-combat-survival.mjs` writes partial reports per beat,
  writes final JSON/Markdown atomically, avoids known Crystal field safe-zone
  circles, and transfers to Woomyon combat anchor `1:315,100`. Evidence
  `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-anchor-20260707/report.md`
  connected to Rust `ws://127.0.0.1:7111/ws` and fought outside safe zone. Red
  results: melee attack packets were sent against `ForestYeti`, but there was
  no `ObjectStruck`, no `DamageIndicator`, no target health drop, and no kill;
  provoking `RakingCat0` did not reduce player HP; `@DIE` did not transition to
  dead/revive. Frontend acceptance should therefore keep damage floaters,
  attack/struck/death animation feel, loot/XP feedback, and death/revive UI in
  `[~]` active status until the gateway/Zone combat outcome is fixed and this
  same evidence path is rerun. Asset gaps from the run: missing
  `original-ui/Sound/103.wav` and missing `Monster/007` original-ui metadata.
- 2026-07-07 combat/effect-heavy probe: default self-camera movement now has a
  follow-up combat/effects evidence lane, and it is currently red. Report
  `docs/generated/player-qa/combat-survival-default-selfcamera-20260707/report.md`
  produced 11 screenshots and 11/11 completed harness beats on the default
  Bevy WebGL2 route, but connected through `ws://127.0.0.1:7110/ws` instead of
  Rust `7111`, failed to reliably reach/engage a hunting-field monster, skipped
  `.scene-damage-floater` because no landed damage was observed, and failed
  death/revive (`@DIE` left the player at `0/18` without a dead-state transition).
  Positive signal: the survival beat did observe player HP falling `18 -> 9`.
  Magic/effect QA is not yet evidence: the attempted
  `magic-skills-default-selfcamera-*` runs currently stall before report
  generation around login/register and need harness repair. Harness follow-up:
  both combat and magic QA scripts now wrap CDP commands in a 15s timeout, so
  future stalls should write a fatal report instead of hanging silently.
- 2026-07-07 default self-camera held/chorded pass: keyboard movement now has
  the same default self-camera evidence style as the Bichon click route. The
  chorded/cardinal capture
  `docs/generated/player-qa/movement-jitter/web-motion-keyseq-bichon-cardinal-default-selfcamera-windowfps-content-jpeg-20260707-2000.json`
  is `ok=true`, 148 JPEG frames, 8 movement commands, final `329,270`, no
  failed assertions, no logical rollback, no interaction pollution, and Bevy
  WebGL2 packed rendering. The first held Shift+Right default capture
  `docs/generated/player-qa/movement-jitter/web-motion-heldrun-bichon-right-default-selfcamera-windowfps-content-jpeg-20260707-2000.json`
  intentionally documents the red repro: movement reached `345,270`, but one
  logical rollback occurred when predicted self position briefly fell from
  `332,270` to the server `331,270` between run ACKs. Fix: fresh unconsumed
  direction `queuedMoveIntent` now counts as self-movement transport evidence,
  so prediction is not cleared during sustained held-key cadence. Verified
  rerun
  `docs/generated/player-qa/movement-jitter/web-motion-heldrun-bichon-right-default-selfcamera-windowfps-content-queuedintentfix-jpeg-20260707-2000.json`
  is `ok=true`, 122 JPEG frames at ~50ms cadence, 8 movement commands, average
  ACK `198.5ms`, max ACK `439ms`, final `345,270`, 0 logical rollback
  warnings, 0 failed assertions, 0 frame capture errors, 0 interaction
  pollution, and no console/network failures. Remaining active gap:
  equal-duration native held/video capture plus combat/effect-heavy scenes.
- 2026-07-07 default self-camera temporal pass: the previous equal-cadence
  Web motion/change-intensity gap is now closed for the current Bichon
  four-click route. Bevy self-camera + per-entity interpolation is requested by
  default and activates only when the Bevy entity/map renderer is live; the DOM
  self overlay cancels the parent camera transform so nameplate/health overlays
  stay pinned instead of jumping. Native evidence
  `docs/generated/player-qa/movement-jitter/original-motion-computeruse-route-bichon-4click-highfps-20260707-2000.json`
  remains `ok=true`, 104 JPEG frames over 5167ms, average sample delta
  `50.17ms`, and 4 native clicks at `51/950/1860/2763ms`. Matching default-URL
  Web content-only evidence
  `docs/generated/player-qa/movement-jitter/web-motion-clicksequence-bichon-samedir-4click-windowfps-content-default-selfcamera-jpeg-20260707-2000.json`
  is `ok=true`, 105 JPEG frames at ~50ms cadence, 4/4 walk ACKs, average ACK
  `139.25ms`, max ACK `369ms`, no visual jumps, no interaction pollution, no
  failed assertions, and no console/network failures. The final high-fps report
  `docs/generated/player-qa/movement-jitter/temporal-native-highfps-route-vs-web-windowfps-content-default-selfcamera-clicksequence-bichon-20260707.md`
  records normalized visual delta/sec `Crystal 63.7831` vs `Web 62` (Web ratio
  `0.972`) and changed-pixel/sec `Crystal 1.718936` vs `Web 1.7788` (Web ratio
  `1.0348`). Remaining active gap: broaden this evidence to held/chorded
  movement plus combat/effect-heavy scenes, then tune HUD/chat temporal polish
  and effect-layer motion.
- 2026-07-07 native/Web 4-click temporal pass: the real-input native evidence
  now covers a sustained four-click route, not only a one-step click. Native
  Computer Use evidence
  `docs/generated/player-qa/movement-jitter/original-motion-computeruse-route-bichon-4click-20260707-2000.json`
  is `ok=true` with 23 captured native frames and 4 real window clicks. Web
  capture now has explicit `clickSequence` support for fixed relative routes.
  The first same-area route
  `docs/generated/player-qa/movement-jitter/web-motion-clicksequence-bichon-4click-left-jpeg-20260707-2000.json`
  is intentionally retained as red pollution evidence because it hit
  `Teleport_Gilbert` and emitted an `interact`; the clean accepted route
  `docs/generated/player-qa/movement-jitter/web-motion-clicksequence-bichon-leftclean-4click-jpeg-20260707-2000.json`
  passed with 29 JPEG frames, 4/4 walk ACKs, average ACK `204.25ms`, max ACK
  `590ms`, 0 frame capture errors, 0 critical console errors, and 0
  interaction pollution. Report
  `docs/generated/player-qa/movement-jitter/temporal-native-computeruse-route-vs-web-clicksequence-bichon-leftclean-20260707.md`
  records aggregate visual delta `Crystal 11.42` vs `Web 10.11` (ratio
  `0.8853`). Remaining active gap: native higher-cadence/video capture and
  exact same clean-route replay before human smoothness acceptance.
- 2026-07-07 native Computer Use frame-cadence pass: the previous native
  synthetic-input blocker is closed for one-step click movement. New script
  `apps/web/scripts/capture-original-computer-use.mjs` drives the native
  `Legend of Mir 2` window through Computer Use and saves screenshots in the
  temporal-report JSON shape. Evidence
  `docs/generated/player-qa/movement-jitter/original-motion-computeruse-click-620-520-20260707-2000.json`
  captured 9 real native frames; matched Web evidence
  `docs/generated/player-qa/movement-jitter/web-motion-clicktarget-bichon-287-611-plus1-left-jpeg-1800ms-20260707-2000.json`
  passed with one clean `walk DownRight`, final `288,612`, 10 JPEG frames, 0
  failed assertions, and 0 interaction pollution. Report
  `docs/generated/player-qa/movement-jitter/temporal-native-computeruse-click-vs-web-clicktarget-bichon-1800ms-20260707.md`
  records native mean visual delta `7.09` / changed-pixel ratio `0.16855`
  versus Web `4.51` / `0.108783`. Remaining active gap: repeat on longer
  route/run samples and improve capture cadence before calling human-feel
  parity accepted.
- 2026-07-07 frame-cadence automation pass: Web movement evidence can now be
  sampled as real full-stage frames instead of relying only on final
  screenshots and movement ACKs. `capture-web-movement-jitter.mjs` supports
  scheduled per-sample frame capture, JPEG frame output, and blank WebGL canvas
  detection/fallback; `report-movement-temporal-parity.mjs` scores
  consecutive-frame visual deltas. Evidence
  `docs/generated/player-qa/movement-jitter/web-motion-keyhold-right-jpeg-cadence-20260707-2000.json`
  passed with 23 JPEG frames, about 98ms average frame-sample spacing, 0 frame
  capture errors, 0 failed assertions, 0 interaction pollution, and final
  player `335,270`. Report
  `docs/generated/player-qa/movement-jitter/temporal-keyhold-native-static-vs-webjpeg-cadence-20260707.md`
  records aggregate visual delta `Crystal 0.37` vs `Web 7.09`; this highlights
  that the current native Crystal synthetic-input sample is not a valid moved
  baseline yet. SendInput scan-code keyboard, right-click target, and
  left-click target probes also stayed near static visual deltas (`0.43`,
  `0.33`, `0.46`). Remaining active gap: automate native Crystal real input or
  video capture so animation cadence can be compared against Web's now-clean
  held-run/JPEG trace.
- 2026-07-07 held/chorded keyboard movement closeout: the first forced WebGL2
  held Shift+Right Bichon repro exposed a backend world-runtime issue rather
  than a frontend renderer hitch. Before the fix,
  `docs/generated/player-qa/movement-jitter/web-motion-heldrun-bichon-right-webgl2-movelog-20260707.json`
  was `ok=false` because the player reached `0:339,270`, hit the leftover
  starter demo transfer, received delayed ACKs `7481/4066ms`, and rolled back
  toward `0:330,270`. After clearing starter transfers from full Crystal world
  runtime,
  `docs/generated/player-qa/movement-jitter/web-motion-heldrun-bichon-right-worldtransferfix-20260707.json`
  is `ok=true` with 8/8 movement ACKs, max ACK 359ms, no logical rollback,
  no stale prediction, no command queue warnings, no interaction pollution,
  and Bevy WebGL2 packed rendering with no DOM entity fallback. The chorded
  cardinal rerun
  `docs/generated/player-qa/movement-jitter/web-motion-keyseq-bichon-cardinal-worldtransferfix-rerun-20260707.json`
  also passed strict checks with all eight ACKs under 300ms. Remaining
  frontend feel gap: native Crystal animation cadence, per-frame sprite timing,
  and camera/HUD temporal polish still need side-by-side recording; static
  screenshots and now-clean movement ACK traces alone do not prove human
  smoothness parity.
- 2026-07-07 crowded Bichon click-route closeout: the earlier Bichon red sample
  was polluted by entity hit targets and then by a Gateway post-ACK scheduling
  race. `capture-web-movement-jitter.mjs` now supports clean route patterns,
  entity-hit avoidance, pollution-fail assertions, and final Bevy renderer
  readiness waits; the self player sprite/nameplate no longer intercepts ground
  movement clicks. Evidence:
  `docs/generated/player-qa/movement-jitter/web-motion-clickroute-bichon-leftclean-postgrace1500-20260707.json`
  is `ok=true` with clean settle, 4/4 ACKs, ACK latencies `490/164/33/5ms`,
  0 entity-hit clicks, 0 non-movement gameplay frames, Bevy WebGL2 packed
  rendering, and no DOM entity fallback. The matching temporal summary is
  `docs/generated/player-qa/movement-jitter/temporal-clickroute-postgrace1500-20260707.md`.
  A repeat capture
  `docs/generated/player-qa/movement-jitter/web-motion-clickroute-bichon-leftclean-postgrace1500-rerun-20260707.json`
  also passed with ACK latencies `582/78/109/7ms`.
- 2026-07-07 movement temporal click-route pass: native Crystal short-sequence
  frame capture is now paired with Web per-sample frame capture and a generated
  temporal summary. `capture-web-movement-jitter.mjs` can save frame images via
  `--captureFrameImages true`, align mouse timing with `--routeStepMs` /
  `--clickHoldMs`, and now filters movement ACK latency against self
  `UserLocation`-class packets instead of other entities' `ObjectWalk` noise.
  A Web input gap was fixed: right-click target movement now primes run
  immediately, closing the previous "right-click route sent only walk packets"
  mismatch. Evidence:
  `docs/generated/player-qa/movement-jitter/original-motion-frames-20260707-183007.json`
  captured 16 native Crystal frames;
  `docs/generated/player-qa/movement-jitter/web-motion-clickroute-runfix-woods-20260707-183748.json`
  passed strict Web click-route checks on WoomyonWoods(S) with `ok=true`, 8/8
  self ACKs, max ACK 301ms, Bevy WebGL2 drawn, 0 critical console errors, and 0
  non-favicon 404s; and
  `docs/generated/player-qa/movement-jitter/temporal-clickroute-runfix-20260707-183748.md`
  summarizes the comparison. Remaining gap: the Bichon crowded route sample
  `docs/generated/player-qa/movement-jitter/web-motion-clickroute-runfix-clean-20260707-183601.json`
  still fails strict ACK responsiveness after the first run due to crowded
  AOI/blocked-route conditions, so Bichon mouse-route feel is not accepted yet.
- 2026-07-07 Crystal/Web same-scene movement/resource clean pass: the local
  Bichon `0:286,610` keyboard sequence capture now suppresses the Web-only
  tutorial, waits for playable scene state, and runs against a resource set
  expanded with Crystal `NPC/09`, `Monster/011`, and `Monster/013`. Evidence:
  `docs/generated/player-qa/movement-jitter/local-crystal-visual-baseline-keyseq-clean-20260707-181953.json`
  passed with `ok=true`, `strictStatus="settled"`, 4/4 movement frames ACKed,
  all 15 movement assertions green, no visual jumps, no logical rollback, no
  residual movement plan, Bevy WebGL2 gameplay layers drawn, 0 critical console
  errors, and 0 non-favicon 404s. This closes the polluted 367-resource-404
  movement sample as a measurement problem; the remaining Crystal "smoothness"
  gap should now be investigated with temporal recording/animation cadence
  comparisons instead of resource-load noise.
- 2026-07-07 Crystal/Web same-scene visual harness refresh: added
  `apps/web/scripts/report-crystal-visual-parity.mjs` and
  `npm run qa:visual-parity` so Windows Crystal screenshots and Web captures
  can be scored as repeatable trend evidence instead of ad-hoc screenshots.
  `capture-crystal-parity.mjs` now waits for visual scene readiness, suppresses
  the Web-only tutorial overlay during parity captures, emits Bevy map/entity
  renderer diagnostics, and separates raw console errors from critical console
  errors. The Web-only top-center objective tracker now defaults off unless
  explicitly enabled with `?objectiveTracker=1` or
  `localStorage["mir2:objectiveTracker"]="1"`, removing the previous P1
  silhouette mismatch while preserving an opt-in onboarding path. Current
  local evidence:
  `docs/generated/player-qa/visual-parity/current-20260707-181734-report.md`
  reports weighted 95%, runtime/layout/entities 100%, pixel trend 86%, and
  estimated human visual/feel parity band 88-100% for Bichon `0:286,610`,
  with no recurring automated top gaps. Remaining visible differences are now
  mostly temporal/state-sensitive: Crystal lighting/shadow timing, animation
  frame mismatch, chat text, and live HP/MP/gold state. Movement/feel still
  requires a separate recording pass; this static visual score must not be used
  to close the "Crystal feels smoother" gap by itself.
- 2026-06-14 gameplay-feel pass (merged to `main`, deployed): floating damage
  numbers + hit flash (Crystal `DamageIndicator`, #98) close the "combat felt
  dead" gap; all Crystal sound effects are wired with faithful triggers (#99);
  ground drops show real item icons with walk-over-to-pick-up (#97); a loading
  overlay replaces the black stage on first entry (#95); movement + map-object
  alpha-keying run off the main thread (#93, #96). The active R2 release
  `mir2/v/20260601-fullcrystal-a2f10be0` is complete (0 missing), so the prior
  sprite-404 storms are resolved and the per-deploy asset-cache wipe is fixed
  (#100). Continuous monster-click AutoHit (chase + re-path + swing) also landed.
- 2026-05-27 NPC input and skill preflight closeout: Player Web now preserves
  server skill cast metadata (`spell`, `castKind`, `offensive`, hotkey/timing)
  from the world snapshot and routes skill clicks by Crystal cast mode instead
  of always sending an opaque `castSkill`. Passive skills are not actively
  cast, target skills require a selected live monster target when offensive,
  ground skills wait for a clicked tile, direction skills use the player's
  facing, and self/toggle skills use the matching magic/toggle packet shape.
  Debug world snapshots now also carry `npcScriptDiagnostics` so admin/debug
  surfaces can inspect script parser/runtime diagnostics. Evidence: Web
  typecheck plus the focused simulation NPC/skill preflight regressions in
  this pass.
- 2026-05-27 Crystal map/minimap/resource parity closeout: scene blueprint
  application now preserves existing `miniMapIndex`/`bigMapIndex` when a
  partial blueprint reports `null`; Bichon map `0` resolves mini and big map
  index `101` from `CRYSTAL_MINI_MAP_TRANSFORMS` rather than relying on the
  respawn manifest; minimap transform map names normalize by basename,
  lowercase, and `.map` stripping; and object-mode original-map frames now use
  exported Crystal frame offsets for all frames with offset metadata. The old
  Bichon torch offset remains only as a starter-JSON fallback for missing
  metadata. Scene asset readiness keys now use a stable visible URL-set hash
  instead of raw player coordinates, reducing per-step preload churn. Evidence:
  `MIR2_CANDIDATE_SCOPE=local bash infra/check-candidate-gate.sh` passed,
  including Web typecheck, movement-controller, minimap-transform,
  resource-loading, focused Rust gateway/simulation/admin checks, and
  `git diff --check`.
- 2026-05-27 Crystal resource loading hardening: Player Web now treats Crystal
  `.Lib` files like indexable MLibrary sources instead of decoding every frame
  during parse. `crystal-map-loader` stores library frame offsets and lazily
  decodes only requested frames behind decoded-frame/server map/server library
  LRU byte caps. Production request paths are read-only by default:
  `MIR2_DISABLE_REQUEST_FILE_WRITES=1` or production without the explicit dev
  opt-in returns a visible `resource_missing` error when a required PNG/lib/map
  is absent, and synthetic map fallback requires
  `MIR2_ALLOW_SYNTHETIC_MAP_FALLBACK=1`. Scene blueprint cache keys are now
  quantized by map/chunk/size bucket/schema version with disk TTL/size trim.
  `sceneAssetReadiness` actually preloads visible center-priority URLs and
  separates `interactionReady` from `visualReady`; runtime metrics now include
  scene cache key, original-map sprite/cell counts, sprite library cache count,
  DOM image count, Bevy atlas bytes, and alpha-keyed blob count/bytes. Evidence:
  `MIR2_CANDIDATE_SCOPE=local bash infra/check-candidate-gate.sh` passed,
  including Web typecheck, `test:movement-controller`,
  `test:resource-loading`, focused Rust gateway/simulation/admin checks, and
  `git diff --check`. Production/browser visual acceptance remains open.
- 2026-05-26 Crystal Movement Authority Convergence v1: Player Web movement
  now treats server `UserLocation`/movement packets and `worldSnapshot` as the
  only sources allowed to write self `world.entities` coordinates. Normal UI
  movement no longer sends debug `moveTo`; tile, direction, keyboard, and
  mouse movement are queued as Crystal `walk`/`run`/`turn` direction packets
  behind one pending self move, 600ms walk/run cadence, render-only local
  prediction, and Crystal run prewarm. ACK or snapshot disagreement clears the
  pending move/prediction and keeps world state at the server coordinate.
  Verification passed `pnpm --dir apps/web run test:movement-controller`,
  `pnpm --dir apps/web exec tsc --noEmit --pretty false`, Rust fmt checks,
  focused `mir2-simulation` Crystal movement tests, and focused
  `mir2-gateway` movement/Zone route tests. Full `mir2-gateway` was attempted
  but manually stopped after an unrelated two-sided trade rollback test ran for
  several minutes without exiting; focused gateway movement coverage passed.
- 2026-05-26 production walk-run-reverse input closeout: the live repro was
  not just a slow ACK; production could intermittently omit or delay the Run
  edge when the player walked, pressed run, then reversed direction quickly.
  Player Web now preserves Shift/run key edges, keeps a one-action reverse
  backlog instead of overwriting the current queued move, upgrades same-direction
  queued Walk to Run, and lets the movement confirm tick drain that backlog
  instead of leaving prediction state behind. The movement capture harness now
  asserts that a declared keyboard sequence really sends the expected
  `walk/run` WebSocket frames. Web deployment
  `dpl_HttHWiP21hufr1d3mm6fMsHNwcmW` is live behind
  `https://mir2.obelisk.build`, paired with UCloud Gateway release
  `20260526T1918CST-move-input-buffer`. Verification passed Web typecheck,
  movement harness syntax, production `/health`, direct Gateway WSS smoke
  `docs/generated/load/remote-move-input-buffer-wss-smoke-20260526.json`, and
  headed Chrome WebGL2 evidence
  `docs/generated/player-qa/movement-jitter/prod-move-input-buffer-walk-run-turn-webgl2-20260526b.json`
  plus faster 180ms stress
  `docs/generated/player-qa/movement-jitter/prod-move-input-buffer-walk-run-turn-fast-webgl2-20260526a.json`.
  Both captures sent `walk Right -> run Right -> walk Left`, settled at
  `332,270 Left`, had no visual/logical rollback, no stale prediction, no
  residual pending plan/queue, raw WebGL2 `renderedLayers=20`, zero critical
  console errors, and zero non-favicon 404s.
- 2026-05-26 production asset-404 and movement-tick closeout: the live console
  spam for `original-map/WemadeMir2/Objects/2652..2661` and
  `Objects23/1418/1420/1423/1425/1429` was caused by incomplete remote asset
  coverage for the active immutable asset prefix plus overly aggressive retry
  behavior. The missing current-scene files were uploaded to R2 under
  `mir2/v/37596e16d64fde7c`, and immutable original-map/original-ui failures
  now negative-cache instead of appending repeated `mir2ImgRetry` cache busters.
  Production web deployment `dpl_8s8BqYBXe5q5DN9jajRUFnFwFwkt` shipped that
  retry hardening. Follow-up headed Chrome evidence first proved resource
  errors were clean but exposed a second Walk ACK at about `1648ms`; that was
  traced to Gateway deferring runtime ticks by the old 1200ms movement input
  grace. Gateway release `20260526T1435CST-move-tick-grace0` is now installed
  on UCloud with default movement input grace `0`. Verification passed
  `cargo +1.89.0 fmt --check -p mir2-gateway`, focused Gateway tick coverage
  locally and on UCloud, public health, WSS smoke
  `docs/generated/load/remote-move-tick-wss-smoke-20260526.json`, and headed
  Chrome production WebGL2 evidence
  `docs/generated/player-qa/movement-jitter/prod-move-tick-grace0-webgl2-existing-20260526.json`
  / `.png` with `ok=true`, direct WSS host
  `wss://165.154.65.136.sslip.io/ws`, raw WebGL2 atlas `renderedLayers=21`,
  two ordered Walk ACKs at `398ms` and `609ms`, clean settle, no critical
  console errors, and no non-favicon 404s. `Objects/289.png` remains a separate
  source-data or map-library mapping gap because that exact file is absent from
  the local source tree too; the new immutable negative cache prevents it from
  becoming a retry storm while the mapping gap is investigated.
- 2026-05-27 production original-asset manifest hardening: Web now generates
  `public/original-asset-manifest.generated.json` during build/test, hashes it
  into `/api/asset-manifest`, and makes `/api/scene/crystal` refuse map frames
  that are neither present locally nor declared in that manifest. The R2 release
  builder stages every manifest-declared `/original-map` and `/original-ui` PNG,
  and the R2 upload workflow can HEAD the final CDN object for each declared
  original asset before accepting the release. Resource tests now lock the
  previously failing `Objects23/1422/1426/1427/1428`, `NPC/16/27/83`, and
  `Monster/000/139` paths plus Bichon scene blueprint frame coverage.
- 2026-05-27 deterministic asset release wiring: `/api/asset-manifest` now
  prefers `MIR2_ASSET_VERSION`, ignores file mtimes for versioning, and uses
  only `original-asset-manifest.generated.json.assetHash` from the original
  asset manifest. The Web Assets R2 workflow resolves
  `MIR2_ASSET_VERSION=${GITHUB_SHA::12}`, stages/uploads/verifies R2 under
  `mir2/v/$MIR2_ASSET_VERSION`, can deploy the `mir2-domain-proxy` Worker with
  the same version, and only then deploys Vercel. The player-domain Worker now
  serves same-origin `/original-map`, `/original-ui`, and
  `/generated/original-map-blend` requests from R2, so Bevy `/original-ui`
  requests do not depend on React image fallback.
- 2026-05-26 raw WebGL2 atlas gameplay closeout: Player Web now has a
  browser-native `WebGl2EntityAtlasLayer` that reuses the existing
  `BevyEntityRenderState`/entity atlas schema and draws atlas-backed entity
  layers into a transparent WebGL2 canvas. In gameplay wiring, WebGPU still
  uses the Bevy entity renderer; forced WebGL2 keeps the Bevy canvas hidden
  for opaque-surface safety and uses the raw WebGL2 atlas layer once an entity
  atlas is active. The raw WebGL2 path now drives atlas warmup through the same
  GPU renderer condition as WebGPU, and initial scene interaction waits for the
  raw atlas to be ready so first movement does not overlap atlas warmup.
  Headed Chrome local gameplay evidence
  `docs/generated/player-qa/movement-jitter/local-webgl2-raw-atlas-gameplay-gated-20260526.json`
  passed with `ok=true`, selected/compiled backend `webgl2`, `canvasHidden=true`,
  `rawWebGl2Enabled=true`, packed prebuilt atlas `starter-bichon-base`,
  `textureReady=true`, `renderedLayers=21`, `skippedLayers=0`, three Walk sends,
  three ordered `UserLocation` ACKs, no camera-offset stair-step warnings, no
  movement queue warnings, no critical console errors, and no non-favicon 404s.
  The deterministic `/qa/webgl2-entity-renderer` probe remains covered by
  `smoke:bevy-runtime-backends`; evidence
  `docs/generated/player-qa/bevy-runtime-backends/local-webgl2-raw-atlas-probe-20260526.json`
  passed with `rawWebGl2ProbeRendered=true`. Production deployment
  `dpl_Q1k4QFSbGigw9gJ64cfBNcAehjEQ` then shipped the same gate plus hosted
  default Gateway targeting to `wss://165.154.65.136.sslip.io/ws`, away from
  the higher-jitter custom-domain `/ws` route. Bundle probing on
  `https://mir2.obelisk.build` found the direct WSS host in the shipped JS and
  no hard-coded `mir2.obelisk.build/ws`. Headed Chrome production evidence
  `docs/generated/player-qa/movement-jitter/prod-webgl2-raw-atlas-gameplay-focused-direct-default3-20260526.json`
  / `.png` passed with `ok=true`, actual WebSocket
  `wss://165.154.65.136.sslip.io/ws`, selected/compiled backend `webgl2`, raw
  WebGL2 `textureReady=true`, `renderedLayers=21`, prebuilt atlas
  `starter-bichon-base`, three Walk ACKs at `93/51/46ms`, clean settle, no
  camera-offset stair-step warnings after headed-window foregrounding, no
  critical console errors, and no non-favicon 404s.
- 2026-05-26 Bevy runtime backend smoke slice: added
  `npm run smoke:bevy-runtime-backends` so WebGPU-first/WebGL2 fallback is no
  longer checked only by ad-hoc page state. The smoke launches real Chrome,
  exercises default backend selection, forced `?bevyBackend=webgl2`, and
  forced `?bevyBackend=webgpu`, waits for post-boot runtime errors, records the
  selected/compiled backend plus fetched runtime package URLs, and fails on
  critical console errors. Local evidence
  `docs/generated/player-qa/bevy-runtime-backends/local-webgpu-webgl2-runtime-20260526.json`
  passed with default and forced WebGPU selecting/compiling `webgpu`, forced
  WebGL2 selecting/compiling `webgl2`, all runtime JS/WASM package fetches
  succeeding, and zero critical console errors. Important renderer constraint:
  WebGL2 remains unsafe as a transparent Bevy/WGPU surface because the local
  browser advertises opaque alpha only; the newer raw WebGL2 atlas probe above
  is the transparent WebGL2 renderer path to harden instead.
- 2026-05-25 production scene-input unlock closeout: the live "walking command
  feels delayed" repro was caused by movement-triggered viewport asset preloads
  toggling the page back into `scene-assets-pending` and making
  `sceneInteractionReady=false` after the first playable scene. Player Web now
  keeps movement interaction unlocked once the first playable scene has ever
  become ready; later viewport preloads continue in the background without
  gating keyboard, pointer, or mobile repeat input. Production deployment
  `dpl_7iG3bPgA7HTxkvEzN4LxP2rmFmFC` is live behind
  `https://mir2.obelisk.build`, and bundle probing confirmed the new logic is
  present while the old `ready`-only gate is absent. Evidence:
  `pnpm --dir apps/web exec tsc --noEmit --pretty false` and
  `pnpm --dir apps/web exec next build` passed locally before source deploy.
  Headed Chrome production evidence
  `docs/generated/player-qa/movement-jitter/prod-scene-input-unlocked2-webgpu-headed-keyboard-a-nosample-hold-20260525.json`
  / `.png` passed with `ok=true`, WebGPU selected, compiled runtime
  `bevy-b9389323fd0dbead`, packed prebuilt atlas active, no DOM entity
  fallback, `sceneInteractionReady=true` while 699 scene assets were still
  background-loading, five held `A` Walk sends at roughly Crystal cadence,
  authoritative `UserLocation` ACKs `343,342,341,340,339`, no critical console
  errors, and no non-favicon 404s.
- 2026-05-25 production starter-transfer movement closeout: the earlier
  `339 -> 330` rollback in live captures was a server/config issue rather than
  a WebGPU, DOM, or atlas renderer issue. The production Gateway had inherited
  the early demo `starter-east-field-gate` same-map transfer through
  `with_crystal_map_runtime()`. Gateway release
  `20260525T0334CST-starter-transfer-cleanup` is active on UCloud; health
  checks and WSS smoke
  `docs/generated/load/remote-starter-transfer-cleanup-wss-smoke-20260525.json`
  passed. Production headed WebGPU packet-walk evidence crossed from
  `0:338,270` through `0:343,270` with ACKs `339..343`, no map-change packet,
  no rollback to `330,270`, WebGPU selected, packed prebuilt atlas active, no
  critical console errors, and no non-favicon 404s. The packet-walk harness
  still reported expected false route-spam/direction-animation warnings because
  it intentionally sends several same-direction packets in one post-action
  sample window; the authoritative packet evidence is clean.
- 2026-05-25 Bevy entity-atlas direct-image slice: the prebuilt atlas path no
  longer decodes `starter-bichon-base.png` into a 4096x4096 canvas and sends
  64MiB of RGBA pixels to wasm. Prebuilt manifest hits now carry `imageUrl`
  through `BevyEntityRenderState`; the Bevy runtime loads that PNG through
  `AssetServer` and binds the resulting image handle to the atlas layout. The
  existing `setMir2EntityRenderAtlas` pixel-upload API remains as the fallback
  for live browser-packed or explicit pixel atlases. Evidence: `cargo +1.89.0
  check --manifest-path apps/game-client/runtime/Cargo.toml --target
  wasm32-unknown-unknown --no-default-features --features webgl2`, the same
  check with `--features webgpu`, `pnpm --dir apps/web run
  runtime:build:release` producing `bevy-b9389323fd0dbead`, `pnpm --dir
  apps/web exec tsc --noEmit --pretty false`, `MIR2_USE_PREBUILT_BEVY_RUNTIME=1
  pnpm --dir apps/web exec next build`, and headed Chrome local WebGPU play
  against `http://localhost:3100/?bevyEntities=1&bevyBackend=webgpu...`.
  Chrome page-asset inventory observed
  `/bevy-runtime/pkg-webgpu/mir2_bevy_runtime.js`,
  `/bevy-runtime/pkg-webgpu/mir2_bevy_runtime_bg.wasm`, and
  `/bevy-entity-atlases/starter-bichon-base.png`. Movement diagnostics
  `docs/generated/player-qa/movement-diagnostics/manual-mplj7xmo-rpw2ln.jsonl`
  recorded 4 keyboard Walk sends, 4 `UserLocation` ACKs, 367-443ms ACK latency
  with 398ms average, final player `328:256`, and 0 anomalies. Remaining work:
  deploy the rebuilt web bundle and rerun production headed Chrome WebGPU
  acceptance on `https://mir2.obelisk.build`.
- 2026-05-25 mobile/touch black-ground guard closeout: a user follow-up crop
  still showed the remaining failure shape where entity sprites and lamps were
  visible over a black ground plane. Source PNG/atlas alpha spot-checks showed
  the sprite alpha channels were present, so the residual risk was still the
  Bevy canvas surface covering the DOM original map on some browser/device
  path. Player Web now treats mobile/touch and explicit
  `?bevyCanvas=0` / `?bevyCanvasHidden=1` as a DOM-entity fallback path: the
  Bevy canvas is hidden in-game and Bevy entity rendering is disabled. Desktop
  WebGPU remains the default experimental Bevy sprite path, while
  `?bevyCanvas=1` / `?bevyEntities=1` can force it back on. The movement QA
  capture script also gained `--finalSceneReadyTimeoutMs` so screenshot evidence
  waits for the post-movement scene asset key to settle before capture.
  Deployment `dpl_8hgZxTUoDTUokZ1tkTkpVQeU2uwf` is live behind
  `https://mir2.obelisk.build`; `/health`, the WebGPU/WebGL2 runtime JS files
  for `bevy-6732ca9f6ab18f6d`, and `/bevy-entity-atlases/manifest.json` all
  returned 200. Production mobile/touch evidence
  `docs/generated/player-qa/movement-jitter/prod-mobile-dom-fallback-canvas-hidden-finalready-20260525.json`
  / `.png` passed with selected/compiled backend `webgpu`,
  Bevy entity renderer `enabled=false`, `canvasHidden=true`, one Walk send, one
  UserLocation ACK, visible ground/entities, no critical console errors, and no
  non-favicon 404s. Production desktop WebGPU evidence
  `docs/generated/player-qa/movement-jitter/prod-desktop-webgpu-transparent-guard-finalready-20260525.json`
  / `.png` passed with Bevy entity renderer `enabled=true`,
  `canvasHidden=false`, `atlasMode="packed"`, `prebuiltHits=2`,
  `lastSource="prebuilt"`, one Walk send, one UserLocation ACK, visible ground,
  no critical console errors, and no non-favicon 404s. Production escape-hatch
  evidence
  `docs/generated/player-qa/movement-jitter/prod-bevy-canvas-off-dom-fallback-finalready-20260525.json`
  / `.png` passed with `?bevyCanvas=0`, `enabled=false`, `canvasHidden=true`,
  one Walk send, one UserLocation ACK, no critical console errors, and no
  non-favicon 404s.
- 2026-05-25 map black-screen transparent-canvas closeout: the black gameplay
  map was not a missing map-resource or atlas decode failure. The DOM original
  map/backdrop layer was rendering underneath the Bevy canvas, while the Bevy
  web surface was composited as opaque black; higher z-index DOM object/entity
  overlays still appeared, making the ground alone look missing. The WebGPU
  runtime now creates a transparent primary window with premultiplied alpha so
  the original map layer remains visible under Bevy entity sprites. Because
  forced WebGL2 only advertised opaque surface support in the local browser,
  WebGL2 fallback now hides the Bevy canvas for original-map gameplay and keeps
  the DOM entity renderer active instead of panicking or covering the map.
  Evidence: `pnpm --dir apps/web run runtime:build:release` produced runtime
  `bevy-6732ca9f6ab18f6d`, `pnpm --dir apps/web exec tsc --noEmit --pretty
  false` passed, local WebGPU capture
  `apps/web/docs/generated/player-qa/movement-jitter/local-transparent-canvas-webgpu-release-20260525.json`
  / `.png` passed with `ok=true`, selected/compiled backend `webgpu`,
  `canvasHidden=false`, Bevy entity renderer enabled, prebuilt atlas hit, one
  Walk send, one UserLocation ACK, no critical console errors, and no
  non-favicon 404s. Local forced-WebGL2 capture
  `apps/web/docs/generated/player-qa/movement-jitter/local-transparent-canvas-webgl2-release-20260525.json`
  / `.png` passed with selected/compiled backend `webgl2`,
  `canvasHidden=true`, Bevy entity renderer disabled for DOM fallback, one Walk
  send, one UserLocation ACK, no critical console errors, and no non-favicon
  404s. Production deployment `dpl_4i4fFrooS8Esuyjh1b1oSb1NCTMb` is live
  behind `https://mir2.obelisk.build`; `/health` returned 200 and both
  `/bevy-runtime/pkg-webgpu/mir2_bevy_runtime.js?v=bevy-6732ca9f6ab18f6d`
  and
  `/bevy-runtime/pkg-webgl2/mir2_bevy_runtime.js?v=bevy-6732ca9f6ab18f6d`
  returned 200 with `x-mir2-asset-cache: bevy-runtime`. Production WebGPU
  evidence
  `docs/generated/player-qa/movement-jitter/prod-transparent-canvas-webgpu-readywait-20260525.json`
  / `.png` passed with selected/compiled backend `webgpu`,
  `canvasHidden=false`, Bevy entity renderer enabled, `atlasMode="packed"`,
  `prebuiltHits=1`, one Walk send, one UserLocation ACK, no critical console
  errors, and no non-favicon 404s. Production forced-WebGL2 evidence
  `docs/generated/player-qa/movement-jitter/prod-transparent-canvas-webgl2-readywait-20260525.json`
  / `.png` passed with selected/compiled backend `webgl2`,
  `canvasHidden=true`, Bevy entity renderer disabled for DOM fallback, one Walk
  send, one UserLocation ACK, no critical console errors, and no non-favicon
  404s.
- 2026-05-25 Bevy entity atlas prebuild/cache slice: Player Web now checks a
  persistent IndexedDB atlas cache, then a prebuilt
  `/bevy-entity-atlases/manifest.json` atlas pack, before falling back to live
  browser packing. Prebuilt atlas pixels are reused within the page so viewport
  changes do not repeatedly decode/read back the same pack. The starter Bichon
  entity pack is generated by
  `npm run assets:bevy-entity-atlas:build`, covers player/NPC plus common
  Bichon monster roots, and emits
  `public/bevy-entity-atlases/starter-bichon-base.png` with 2,631 source rects
  in a 4096x4096 PNG. Local evidence:
  `docs/generated/player-qa/movement-jitter/local-atlas-prebuilt-postcache-order-a-20260525.json`
  passed on WebGPU with `ok=true`, `sceneInteractionReady=true`,
  `atlasMode="packed"`, 700 active atlas sources, `builds=0`,
  `prebuiltHits=2`, `lastSource="prebuilt"`, one Walk send, one UserLocation
  ACK, no critical console errors, and no non-favicon 404s. Fallback evidence:
  `docs/generated/player-qa/movement-jitter/local-atlas-prebuilt-webgl2-20260525.json`
  passed with forced `bevyBackend=webgl2`, `builds=0`, `prebuiltHits=2`, and
  `lastPrebuiltKey="starter-bichon-base"`. Production deployment
  `dpl_C8sriwUxAeuCyzoY9rAnd24QTw6D` is live behind
  `https://mir2.obelisk.build`; `/bevy-entity-atlases/manifest.json` reports
  `sourceCount=2631`, `imageBytes=4272109`, and `rgbaBytes=67108864`, and the
  PNG returns 200 with `content-length: 4272109`. Production movement evidence:
  `docs/generated/player-qa/movement-jitter/prod-atlas-prebuilt-keyboard-final-20260525.json`
  passed keyboard movement with `builds=0`, `prebuiltHits=1`,
  `lastSource="prebuilt"`, one Walk send, one UserLocation ACK, no rollback,
  no route spam, no critical console errors, and no non-favicon 404s. Mobile
  hand-feel evidence:
  `docs/generated/player-qa/movement-jitter/prod-atlas-prebuilt-mobile-pixelcache-20260525.json`
  passed mobile joystick movement with `atlasMode="packed"`,
  `atlasPendingKey=null`, `builds=0`, `prebuiltHits=1`,
  `lastPrebuiltKey="starter-bichon-base"`, one Walk send, one UserLocation
  ACK, and the same clean assertion set.
- 2026-05-25 WebGPU-first Bevy runtime support: Player Web now builds and
  publishes separate Bevy wasm runtime packages for WebGPU and WebGL2. The
  loader prefers WebGPU on secure browsers with `navigator.gpu`, falls back to
  WebGL2 when WebGPU is unavailable or init fails, and supports explicit
  `bevyBackend=webgpu|webgl2` query/localStorage overrides for diagnostics.
  Runtime debug state is exposed through `window.__mir2BevyRuntimeDebug` and
  included in movement captures. Evidence: `cargo +1.89.0 check
  --manifest-path apps/game-client/runtime/Cargo.toml
  --target wasm32-unknown-unknown --no-default-features --features webgl2`,
  the same check with `--features webgpu`, `pnpm --dir apps/web
  runtime:build:release`, Web typecheck, and headed Chrome local verification
  against `http://127.0.0.1:13014/`: default selected compiled WebGPU,
  forced `bevyBackend=webgl2` selected compiled WebGL2, and a simulated
  missing `navigator.gpu` browser fell back to WebGL2. Screenshot evidence is
  `output/playwright/mir2-webgpu-runtime.png`. Production deployment
  `dpl_HNZTKmg7jPkNju3GhJgAdzk3N9oV` is live behind
  `https://mir2.obelisk.build`; direct runtime probes for
  `/bevy-runtime/pkg-webgpu/mir2_bevy_runtime.js` and
  `/bevy-runtime/pkg-webgl2/mir2_bevy_runtime.js` return 200 with
  `x-mir2-asset-cache: bevy-runtime`. Headed Chrome production movement
  evidence
  `docs/generated/player-qa/movement-jitter/live-webgpu-keyboard-after-gateway-20260525.json`
  passed with `ok=true`, selected/compiled backend `webgpu`, WebGPU and WebGL2
  support visible in runtime debug, zero visual jumps, zero route-spam
  warnings, zero logical rollback, zero direction-lag warnings, responsive
  movement queue, clean settle, no critical console errors, and no
  non-favicon 404s. After the follow-up Gateway release
  `20260525T0630CST-zone-magic-mp-cooldown`, the same headed Chrome WebGPU
  movement gate passed again at
  `docs/generated/player-qa/movement-jitter/live-webgpu-keyboard-after-magic-mp-20260525.json`.
  Screenshot evidence is the adjacent `.png`. Remaining frontend risk: the
  first cold entity atlas can still warm in DOM fallback mode, so atlas
  cache/offline pack hardening remains the next renderer optimization.
- 2026-05-25 production Bevy WebGL2 entity-atlas hardening closeout: the
  visible entity sprite renderer is now deployed behind
  `https://mir2.obelisk.build` on Vercel deployment
  `dpl_4PXPyp3VuAT7vHRQr4ueKBTikbtU`. The atlas source set is hardened against
  common movement animation churn by preloading standing/walking/running frames
  for the current action direction, plus all eight player directions, so
  keyboard movement no longer switches from a standing atlas key to a walking
  atlas key mid-action. While a cold atlas is still building, DOM entity sprites
  remain visible as a fallback; once the packed atlas is active, DOM entity
  sprites are hidden and Bevy owns the body/hair/weapon layers. Verification:
  Web typecheck and scoped diff checks passed, public `/health` returned 200,
  production capture
  `docs/generated/player-qa/movement-jitter/prod-bevy-atlas-dir-20260525T043729.json`
  passed with `ok=true`, `atlasMode="packed"`,
  `atlasCurrentKey="entity-atlas-1iogxdg"`, `atlasPendingKey=null`,
  `atlasCachedCurrent=true`, `atlasLatestCurrent=true`,
  `domEntityFallback=false`, 584 atlas sources, two keyboard Walk sends, two
  `UserLocation` ACKs, no non-favicon 404s, and no critical console errors.
  Screenshot evidence is the adjacent `.png`. A headed Chrome production
  hand-feel pass also entered the live game, moved `Scout` with keyboard input
  to `312:249`, and saved
  `docs/generated/player-qa/movement-jitter/headed-chrome-prod-bevy-atlas-final-20260525T0439.png`.
  Remaining renderer optimization: the first cold production atlas build is
  still expensive (`lastBuildMs=54672` for 584 sources), so a future slice
  should move toward a prebuilt/offline entity atlas or narrower CDN-warmed
  pack strategy.
- 2026-05-25 Bevy WebGL2 packed entity-atlas renderer slice: Player Web now
  has a local-verified path that renders visible entity body/hair/weapon sprite
  layers through the Bevy canvas instead of DOM image stacks, while keeping the
  React map, HUD, hit boxes, nameplates, health bars, and quest markers in the
  existing UI layer. The frontend builds a packed RGBA atlas from the current
  visible entity frames, uploads it to the wasm runtime through
  `setMir2EntityRenderAtlas`, and sends layer state through
  `setMir2EntityRenderState`; the Bevy runtime ingests atlas pixels into an
  `Image` asset and renders sprite layers with `TextureAtlas` indices. Toggles:
  `?bevyEntities=1` / `?bevyEntities=0`, `?bevyAtlas=1` / `?bevyAtlas=0`,
  plus matching localStorage overrides. Evidence: `pnpm --dir apps/web
  runtime:build:release`, `cargo +1.89.0 check --manifest-path
  apps/game-client/runtime/Cargo.toml`, `pnpm --dir apps/web exec tsc --noEmit
  --pretty false`, `node --check
  apps/web/scripts/capture-web-movement-jitter.mjs`, and local capture
  `docs/generated/player-qa/movement-jitter/local-bevy-atlas-chain-20260525.json`
  / `.png` passed with `ok=true`, Bevy entity renderer
  `{ready:true, enabled:true, entityCount:19, layerCount:21,
  atlasMode:"packed", atlasCount:1}`, scene assets `185/185`, no critical
  console errors, and no non-favicon 404s. This is not yet a production deploy
  or full all-asset/offline atlas rollout; next slice is live Chrome feel and
  broader atlas cache/perf tuning.
- 2026-05-25 production asset-domain CORS closeout: the live browser CORS
  error for
  `assets.mir2.obelisk.build/mir2/v/37596e16d64fde7c/original-map/WemadeMir2/Objects/2136.png`
  was caused by a cached asset-domain response missing
  `Access-Control-Allow-Origin`, not by a missing PNG. The R2 asset-cache
  Worker now reapplies `access-control-allow-origin: *`,
  `access-control-allow-methods`, `access-control-allow-headers`, exposed
  headers, and `alt-svc: clear` to Cache API HIT responses as well as fresh R2
  responses. Worker version `ea9ec199-d3e4-4627-a57a-c677ddd426be` is live on
  `assets.mir2.obelisk.build/*`. Evidence:
  `docs/generated/remote-assets/cors-asset-worker-20260525.json` passed GET,
  cache-busted GET, HEAD, and OPTIONS probes from
  `Origin: https://mir2.obelisk.build`; the normal GET remains an
  `x-mir2-edge-cache=HIT` response and now includes
  `access-control-allow-origin: *`.
- 2026-05-25 map-object CORS/canvas hardening: scene map-object `<img>`
  elements now set `crossOrigin="anonymous"` before the existing black
  alpha-key canvas pass reads pixels. This complements the live asset-domain
  CORS Worker fix and prevents cross-origin object sprites from tainting the
  alpha-key canvas when assets are served from `assets.mir2.obelisk.build`.
  Evidence: the reported `Objects/2136.png` URL currently returns 200 with
  `access-control-allow-origin: *`, `access-control-allow-methods`, and exposed
  headers, and `pnpm --dir apps/web exec tsc --noEmit --pretty false` passed.
- 2026-05-24 production movement command-log and hydration closeout: the
  frontend did send movement commands, but production console output did not
  show them because movement diagnostics were only retained in internal debug
  arrays. Player Web now emits `[mir2-move:send]`, `[mir2-move:ack]`, and
  correction logs when `?movementLog=1`, `?moveLog=1`,
  `?movementConsole=1`, `window.__mir2MovementLogEnabled`, or
  `localStorage["mir2-movement-log"]="1"` is enabled, and the movement harness
  captures those console events. The React #418 path was also mitigated by
  marking the app document `notranslate`/`suppressHydrationWarning` and making
  the original-client random overlay name deterministic across hydration.
  Production deployment `dpl_BommXyKsMcAX3Lmw4TYcg82a7Rsw` is live behind
  `https://mir2.obelisk.build` and now bakes
  `NEXT_PUBLIC_MIR2_GATEWAY_WS_URL=wss://165.154.65.136.sslip.io/ws`. Evidence:
  Web typecheck, `node --check apps/web/scripts/capture-web-movement-jitter.mjs`,
  scoped diff checks, public `/health`, and production browser capture
  `docs/generated/player-qa/movement-jitter/prod-normal-directws-keyboard-d-20260524T1513.json`
  with `ok=true`, actual WebSocket `wss://165.154.65.136.sslip.io/ws`,
  six `walk Right` sends, six `UserLocation` ACKs, ACK frame latencies
  `555/522/516/523/517/517ms`, 12 movement console events, zero visual jumps,
  zero logical rollback, zero scene blackouts, clean settle, no critical
  console errors, and no non-favicon 404s. Screenshot evidence is the adjacent
  `.png`.
- 2026-05-24 production Chrome movement renderer closeout: a real Chrome tab
  against `https://mir2.obelisk.build` reproduced the user-visible failure as
  a browser renderer/main-thread runaway during held movement, not as a server
  rollback: the stuck tab reached a 400%+ renderer and 100% Chrome main-process
  CPU before restart. The frontend now caches the original-map region cell
  lookup and only rebuilds viewport map sprites on tile/scene-frame/map-region
  changes, so pixel-interpolated movement frames no longer rescan the full
  original-map region every RAF. Production deployment
  `dpl_FW2JQim28WxQTXsYahXjfFzv1Z7c` is live behind
  `https://mir2.obelisk.build`; `/health` returns 200. Verification passed
  `pnpm --dir apps/web exec tsc --noEmit --pretty false`,
  `node --check apps/web/scripts/capture-web-movement-jitter.mjs`, production
  Vercel build/deploy, real Chrome held-`D` movement from `323:264` to
  `327:264` without another unresponsive-page dialog, and production movement
  capture
  `docs/generated/player-qa/movement-jitter/prod-after-map-sprite-cache-d-hold-20260524T1433.json`
  with `ok=true`, zero visual jumps, zero route spam, zero logical rollback,
  zero direction lag, zero stale prediction warnings, responsive command queue,
  continuous camera offset, clean settle, no scene blackouts, no critical
  console errors, and no non-favicon 404s. Screenshot evidence is the adjacent
  `.png`.
- 2026-05-24 production Web movement rollback correction: Web self prediction no longer
  commits predicted coordinates into authoritative `world.entities`; prediction
  remains a render-only ActionFeed/local-anchor layer until server
  `UserLocation` confirms or corrects it. Prediction also waits for server ACK
  when the original map region is not loaded, the next step is outside the
  loaded region, or the loaded cell is blocked, so server-side collision
  rejections do not first draw the player onto an invalid tile. Evidence:
  `pnpm --dir apps/web exec tsc --noEmit --pretty false`, scoped
  `git diff --check`, and local movement smoke
  `docs/generated/player-qa/movement-jitter/local-left-walk-wait-map-20260523T233000.json`
  passed with `ok=true`, zero visual jumps, zero logical rollback, zero
  scene-layer blackouts, clean settle, no critical console errors, and no
  non-favicon 404s. Production deployment
  `dpl_3BwwKyjXY9UFZS3jSZk3vCsybCrW` is live through
  `https://mir2.obelisk.build`; production smoke
  `docs/generated/player-qa/movement-jitter/prod-left-walk-web-rollback-fix-20260524T0034.json`
  passed with the same movement assertions. Caveat: the final production
  sample had `sceneAssetReadiness.status=loading`, so this row is a movement
  rollback gate; resource readiness remains tracked by the dedicated resource
  smoke entries. After the matching Gateway release
  `20260524T0310Z-rollbackfix` was installed, production Web smoke
  `docs/generated/player-qa/movement-jitter/prod-left-walk-gateway-rollbackfix-20260524T0320.json`
  also passed with `ok=true`, zero visual jumps, zero logical rollback, zero
  scene blackouts, no critical console errors, and no non-favicon 404s.
- 2026-05-23 production Crystal action-queue closeout: the movement pipeline is
  now deployed on remote Gateway release `20260523T071900Z-actionqueue` and
  Player Web action-queue verification deployment `dpl_HmHQ4CXfy7d895kHFMfiNLHWespN`, with custom-domain `https://mir2.obelisk.build/health` passing. Web self movement is driven by local
  `QueuedAction`/ActionFeed state, treats self `UserLocation` as ACK/correction
  instead of a new animation source, renders two-tile Run in one Crystal 600ms
  action window, caps local ActionFeed lead to two tiles, and treats
  non-matching `UserLocation` as correction instead of a stale echo.
  Production walk evidence
  `docs/generated/player-qa/movement-jitter/prod-action-queue-keyboard-walk-fix2-20260523T1331.json`
  and run evidence
  `docs/generated/player-qa/movement-jitter/prod-action-queue-keyboard-run-fix2-20260523T1332.json`
  both report `ok=true`, zero visual jumps, zero logical tile rollback, zero
  scene-layer blackouts, responsive movement queue, clean settle, no critical
  console errors, and no non-favicon 404s. Screenshots are the adjacent `.png`
  files.
- 2026-05-23 Chrome manual movement/NPC-click follow-up: direct control of
  the live Chrome game tab confirmed that the player now loads and cycles real
  Crystal walk/run frames during manual movement. A clean right-click route
  around Bichon advanced from `327:271 -> 328:270 -> 330:268` and used
  `CArmour/00` walk frames `38-43` followed by run frames `86-91`; a left-click
  walk to `331:268` used walk frames `44-49`. No scene-layer blackouts were
  observed in the live tab. The sticky-feel issue reproduced around the Bichon
  NPC cluster was a frontend Crystal mismatch: `handleViewportTileAction`
  previously activated the nearest NPC when the target tile was merely near an
  NPC and the player was within interaction range. Crystal only suppresses
  movement when the actual clicked object is an NPC/player/special monster.
  That radius-based `nearbyNpc` shortcut has been removed, so empty ground near
  NPCs remains a movement target. Web typecheck passed after the fix:
  `pnpm --dir apps/web exec tsc --noEmit --pretty false`. A production harness
  run, `docs/generated/player-qa/movement-jitter/prod-manual-click-run-open-20260523-analysis.json`,
  is clean for blackouts/404s/console errors but did not emit a player movement
  command, so the manual Chrome sample is the movement evidence for this item.
- 2026-05-23 production scene-blackout follow-up: the user-reported movement
  flicker where the main scene went black while HUD/minimap/chat stayed visible
  was caused by the `scene-assets-pending` CSS state hiding all primary scene
  layers (`game-scene-backdrop`, sprite overlay, entity overlay, and drop
  overlay) with `opacity: 0` while movement-triggered scene asset readiness was
  loading. The fix keeps the previous scene visible during pending asset checks
  and only disables the scene grid pointer target. The production deployment
  is `dpl_5J4k5qF8mAbnjoj79gGYw2ypZTNv`, visible through
  `https://mir2.obelisk.build`. Verification passed Web typecheck, movement
  harness syntax, scoped diff check, production `/health`, direct production
  probes for `NPC/83/1.png`, `Monster/010/3.png`, `Title/321.png`, and Bevy
  wasm, plus the production keyboard movement capture
  `docs/generated/player-qa/movement-jitter/prod-scene-blackout-normal-walk-20260523134030.json`.
  That capture reports `ok=true`, `noSceneLayerBlackouts.count=0`,
  no visual jumps, no route spam, no logical tile rollback, no direction lag,
  stale prediction cleared, command queue responsive, clean settle, no critical
  console errors, and no non-favicon 404s. Screenshot evidence is
  `docs/generated/player-qa/movement-jitter/prod-scene-blackout-normal-walk-20260523134030.png`.
- 2026-05-22 production movement/resource closeout: the live Gateway rollback
  cause was a shared-zone snapshot merge overwriting the Zone-authoritative
  player transform with the stale personal `SimulationSession` transform on the
  same map. The remote Gateway was rolled forward to
  `20260522T174413Z-zone-transform`, and Player Web was deployed through
  Vercel production deployment `dpl_BHimAGw5LRUVHUTFaWSUZsGhf2AH`. Frontend
  follow-up fixed self `UserInformation` class/gender sprite hydration, scaled
  movement animation lifetime by tile distance, preloaded the whole current
  entity action frame set, removed the stale scene-readiness ready/loading
  loop, and made transient sprite metadata failures retry instead of silently
  dropping CArmour/CHair body layers. Evidence: Web
  `pnpm --dir apps/web exec tsc --noEmit --pretty false`, script syntax
  checks, production `/health`, Gateway public `/health`, and direct production
  probes for `CArmour/00`, `CHair/00`, `NPC/83/1.png`, and
  `Monster/010/3.png` all passed. Final production keyboard movement capture
  `docs/generated/player-qa/movement-jitter/prod-zone-transform-sprite-retry-2m-20260522.json`
  reports `ok=true`, `noVisualJumps.count=0`, no route spam, no logical tile
  rollback, no direction lag, stale prediction cleared, command queue
  responsive, clean settle, no critical console errors, no non-favicon 404s,
  and 186/186 scene assets loaded. Screenshot evidence is
  `docs/generated/player-qa/movement-jitter/prod-zone-transform-sprite-retry-2m-20260522.png`;
  the self player, NPC bodies, and monster bodies are visibly rendered.
- 2026-05-22 production long-session movement/resource follow-up: the
  user-reported Chrome resource errors and non-smooth movement were retested
  against `https://mir2.obelisk.build` with real production login and keyboard
  movement. Landed fixes split WebSocket keepalive from QA-only `autoTick`,
  relaxed movement prediction waiting to exact occupied path tiles, held
  turn visuals for a full Crystal action frame, suppressed repeated held
  blocked-direction attempts, and deployed Cloudflare domain/R2 Workers with
  asset proxy response cleanup. Vercel production deployment
  `dpl_8NeUFDsKu2NKMTFuAf1yF9YEoxXV` is promoted current and `/health`
  returns 200. Evidence:
  `docs/generated/player-qa/movement-jitter/prod-movement-fix-15m-20260522.json`
  ran for 15 minutes with `packetRuntimeModes={"packetRefresh":3513}` and no
  reconnect samples, clean settle, no residual `predictedPlayer`,
  `movementPlan`, `directionStepPendingQueue`, or
  `outstandingSelfMovementActions`, 196/196 scene assets loaded, and no
  non-favicon 404s. It improved but did not close all movement-feel checks:
  visual jumps 156, logical tile rollback 41, route spam 5, direction lag 4,
  and console errors 31. All console errors were
  `net::ERR_QUIC_PROTOCOL_ERROR`; direct probes for affected URLs returned
  200, and `NPC/83/1.png` returned 200 through the R2-backed player-domain
  proxy. Cloudflare still injects `alt-svc: h3`; Worker/Next response headers
  cannot override that edge setting, and the current Wrangler OAuth token can
  deploy Workers but receives 403 for zone setting `http3`. Follow-up:
  disable Cloudflare HTTP/3/QUIC for `obelisk.build` with a zone-settings token
  or dashboard access, then rerun the production movement/resource capture.
- 2026-05-22 production movement rollback/smoothness pass: rapid opposite
  direction input no longer lets locally predicted `worldRef` position clear
  direction-step pending state as though it were a server acknowledgement. The
  settlement path now uses `lastSelfMovementAck` while packet-transport
  movement evidence is active, so old local prediction cannot prematurely clear
  pending movement or snap the self sprite across tiles. The production baseline
  `docs/generated/player-qa/movement-jitter/prod-movement-baseline-fresh-20260522-173704.json`
  reproduced the issue with `ok=false` and `noVisualJumps.count=1`. After the
  fix, Web `pnpm --dir apps/web exec tsc --noEmit --pretty false` passed and
  production deployment `dpl_xryqwBF4NVPh7KdNio2ppv6EFPYh` is visible through
  `https://mir2.obelisk.build`. The production movement capture
  `docs/generated/player-qa/movement-jitter/prod-movement-fix-keyseq-20260522-180533.json`
  reports `ok=true`, `noVisualJumps.count=0`, no logical tile rollback, clean
  movement settle, stale prediction cleared, responsive movement command queue,
  running animation state present, no console errors, and no non-favicon 404s;
  screenshot evidence is
  `docs/generated/player-qa/movement-jitter/prod-movement-fix-keyseq-20260522-180533.png`.
- 2026-05-21 production map-monster screenshot pass: a production-safe QA
  screenshot surface now covers original map terrain plus Crystal respawn data
  without exposing debug player teleport commands. `/api/qa/map-monster-scenes`
  enumerates 807 representative scenes from the Crystal respawn manifest,
  covering 463 source maps, 284 maps with positive respawns, and 6340 positive
  respawn rows. `/qa/map-monsters` renders each scene through
  `loadCrystalSceneBlueprint`, using the loader-clamped `sceneView.center` for
  sprite placement and clamped labels for respawns whose source coordinates sit
  outside the renderable map bounds. The capture tool now accepts exact
  `--sceneIndexes` so production retakes can replace only failed scenes.
  Production deployment `dpl_9L3LsRnN8mfJmDirFCpjnrBdeNJR` is READY at
  `https://mir2-web3-7ov6lp1xs-obelisk-labs.vercel.app` and visible through
  `https://mir2.obelisk.build`. Evidence:
  `docs/generated/player-qa/production-map-monsters/production-full-map-monsters-qa807-resource-strict-20260521/summary.aggregate.json`
  reports `ok=true`, 807/807 final captured scenes, failure count 0,
  zero-map-sprite scenes 0, broken images 0, network 404s 0, network failures
  0, and console errors 0. The aggregate combines the full run, 38 scene
  retakes after the render-center fix, a focused GA1 retake, and 44 low
  concurrency retakes that removed high-concurrency resource load noise. It is
  the production resource-health gate rather than a guarantee that every
  heavy-map high-concurrency screenshot reached `imagesComplete=true`; for
  complete-pixel visual retakes the capture script now has QA-only
  `--fulfillOriginalMapFromPublic`, which still opens the production QA URL
  while fulfilling original-map requests from the restored local release. A
  focused `hyunwol1` retake with that mode verified `imagesComplete=true`,
  `pendingImageCount=0`, 588 rendered map images, and a nonblank terrain
  screenshot. The only durable missing asset found in that process was GA1
  `WemadeMir2/Objects10`
  frames; 27 frames `5172..5234` were exported from the full Crystal
  `Objects10.Lib`, uploaded to R2 prefix `mir2/v/37596e16d64fde7c` via
  `docs/generated/remote-assets/prod-ga1-objects10-patch-20260521/remote-asset-release.json`,
  direct CDN probes returned 200, and the GA1 retake finished with 99 map
  images, 0 pending, 0 broken images, 0 404s, and 0 console/network failures.
- 2026-05-21 production original-map runtime-data pass: representative live
  maps were rendering as flat fallback terrain because `/api/scene/crystal`
  could not read the full Crystal `Map/` and `Data/Map/*.Lib` source tree in
  Vercel, so it fell back to the packaged starter Bichon fragment or empty
  map regions. `crystal-map-loader.ts` now uses the local full-client root
  when available, and production falls back to generated compressed runtime
  map data under `lib/generated/crystal-map-pack` plus frame dimensions under
  `lib/generated/crystal-map-library-meta`. The scene blueprint cache schema
  was bumped so old fallback regions are not reused. Generated runtime data
  covers 1624 Crystal map files and 138 map libraries / 1,327,368 frame
  metadata entries, and the newly needed rendered PNG frames were uploaded to
  the active R2 release. Production deployment
  `dpl_CLp4KrpvspZaPHExjdjtazkRdFUs` is READY at
  `https://mir2-web3-5kzhyxrns-obelisk-labs.vercel.app`, aliased to
  `https://mir2-web3-web.vercel.app`, and visible through
  `https://mir2.obelisk.build`. Evidence: `pnpm --dir apps/web exec tsc
  --noEmit --pretty false`, `node --check
  apps/web/scripts/generate-crystal-map-runtime-data.mjs`, and focused diff
  whitespace checks passed. Direct production probes returned non-empty
  regions for `0@271,259` (697 sprites / 4116 cells), `1@308,170`
  (132 / 2503), `D011@206,206` (231 / 5581), `D401@106,106`
  (397 / 5969), `D2042@156,56` (563 / 5169), and `D5063@39,15`
  (96 / 5070); previously missing sample images such as
  `/original-map/WemadeMir2/Tiles/1950.png` and
  `/original-map/WemadeMir2/Objects23/1966.png` returned 200. Playable
  Bichon screenshot evidence
  `docs/generated/player-qa/live-map-monsters/prod-map0-bichon-runtime-wait20-20260521Tnow.png`
  recorded `mapObjectSpriteCount=120` and `network404Count=0` where the prior
  live capture at the same coordinate had 0 map object sprites; the capture
  waits for the larger production map image set to settle. Production
  cross-map screenshot automation is intentionally blocked by the production
  player-command safety rule rejecting debug `crystal:<map>:<x>:<y>` transfer
  keys, so cross-map visual proof is API-level until a non-debug map routing
  or admin QA relocation path is available.
- 2026-05-21 original-ui metadata/exporter split pass: `/api/original-ui-meta`
  no longer imports `lib/original-ui-export-server.ts` in the production route.
  The route now reads already deployed/static `meta.json` from the app/player
  domain or configured R2/CDN base through
  `lib/original-ui-meta-server.ts`, and missing metadata returns
  `library_not_deployed` instead of doing request-time Crystal export. This
  removes the production build trace that scanned `public/original-ui`.
  Evidence: Web `pnpm --dir apps/web exec tsc --noEmit --pretty false`
  passed; Vercel production build emitted no
  `original-ui-export-server.ts` broad-pattern warning, leaving only the
  separate `crystal-map-loader.ts` / `public/original-map` warning. Production
  deployment `dpl_Fq8FkQb2JxjEmMAHwNXJCU4v7Xdi` is READY at
  `https://mir2-web3-ezaeeogvv-obelisk-labs.vercel.app`, aliased to
  `https://mir2-web3-web.vercel.app`, and visible through
  `https://mir2.obelisk.build`. Direct probes returned 200 for
  `/api/original-ui-meta?library=Items`, `/api/original-ui-meta?library=NPC/94`,
  representative R2-backed asset paths, debug samples, and Bevy wasm; invalid
  `Map/foo` returned `unsupported_library`. Production cache-maintenance smoke
  `docs/generated/player-qa/cache-metrics/cache-metrics-meta-reader-split-prod-20260521.json`
  passed with `ok=true`, 387/387 prewarm ok, warm transfer 0 bytes, and no
  non-favicon 404s. Playable production smoke
  `docs/generated/player-qa/cache-metrics/cache-metrics-meta-reader-split-playable-prod-20260521.json`
  passed with `ok=true`, cold first playable 13745.3ms, warm first playable
  14118.8ms, 387/387 prewarm ok, and no non-favicon 404s.
- 2026-05-21 CDN-first Vercel output pass: Player Web production deployment
  now keeps the Vercel artifact focused on the Next.js shell, route handlers,
  retained debug samples, and same-origin Bevy runtime, while Crystal
  `/original-ui`, `/original-map`, and `/generated/original-map-blend` media
  are served from the verified R2/CDN release through the player domain.
  `apps/web/scripts/prune-vercel-output-assets.mjs` prunes only those
  R2-backed paths from `.vercel/output` after `vercel build`; the final report
  `docs/generated/remote-assets/vercel-output-prune-resource-cdn-first-20260521.json`
  reduced output size from 420,957,251 bytes / 18,650 files to 43,478,680 bytes
  / 278 files. Production deployment `dpl_ieQqdaZMnnZYNe4wxksuoqsj7Sgg` is
  READY at `https://mir2-web3-js3ofmmod-obelisk-labs.vercel.app`, aliased to
  `https://mir2-web3-web.vercel.app`, and visible through
  `https://mir2.obelisk.build`; upload size was 15.7MB. Direct probes returned
  200 for R2-backed title/item/map/blend assets, retained
  `/debug/map-samples/smtile-72.png` and `smtile-80.png`, and same-origin Bevy
  wasm. Evidence: `node --check
  apps/web/scripts/prune-vercel-output-assets.mjs`,
  `pnpm --dir apps/web exec tsc --noEmit --pretty false`, production
  cache-maintenance smoke
  `docs/generated/player-qa/cache-metrics/cache-metrics-resource-cdn-first-final-prod-20260521.json`
  with `ok=true`, 387/387 prewarm ok, warm transfer 900 bytes, no critical
  console errors, and no non-favicon 404s, plus playable production smoke
  `docs/generated/player-qa/cache-metrics/cache-metrics-resource-cdn-first-playable-final-prod-20260521.json`
  with `ok=true`, cold first playable 14212.5ms, warm first playable 14163.9ms,
  warm transfer 600 bytes, and no non-favicon 404s.
- 2026-05-21 resource cache-tier production pass: Player Web resource
  management now separates declared static asset packs into Service Worker
  cache tiers instead of one bulk static cache. `/api/asset-manifest` exposes
  per-tier budgets (`staticCriticalMaxEntries=3000`,
  `staticBackgroundMaxEntries=6000`, `staticRuntimeMaxEntries=16000`);
  `login`, `character-select`, and `hud-core` are tagged
  `cacheTier=critical`; `bichon-spawn` is tagged `cacheTier=background`;
  scene-frame prewarm sends best-effort tier hints so dynamic Bichon frames
  populate `mir2-asset-cache-static-background-*` while login/select/HUD stay
  in `mir2-asset-cache-static-critical-*`. Evidence: Web
  `node --check apps/web/public/mir2-asset-worker.js`,
  `node --check apps/web/scripts/smoke-cache-metrics.mjs`,
  `pnpm --dir apps/web exec tsc --noEmit --pretty false`, and
  `pnpm --dir apps/web run build` passed. Local production Web
  `127.0.0.1:13021` passed
  `docs/generated/player-qa/cache-metrics/cache-metrics-resource-tier-local-20260521.json`
  with `ok=true`, 387/387 prewarm ok, warm CacheStorage 3 caches / 383
  entries / 63.9MB, no critical console errors, and no non-favicon 404s.
  Production deployment `dpl_9qZP7jXVU1Q6BzUWZVyQKKkMgiaf` is READY at
  `https://mir2-web3-aefb2e729-obelisk-labs.vercel.app`, aliased to
  `https://mir2-web3-web.vercel.app`, and visible through
  `https://mir2.obelisk.build`; production manifest version
  `5d1ec8e93c1caa62` reports the new tier budgets and cache tags.
  Production smoke
  `docs/generated/player-qa/cache-metrics/cache-metrics-resource-tier-prod-20260521.json`
  passed with `ok=true`, 387/387 prewarm ok, warm CacheStorage 3 caches / 383
  entries / 51.1MB, after-cleanup caches
  `static-critical`, `static-background`, `scene`, and `api`, reset deleted 4
  caches/unregistered 1 scope, and all cache-budget, console, and network
  assertions true.
- 2026-05-21 production map-change entity cleanup: Player Web now treats
  `MapInformation` with a different map file as a hard scene boundary and
  clears old non-self entities, drops, projectiles, terrain/decor, selection,
  and active NPC dialog before applying the new map's object packets. This
  closes the live QA state where switching a production test account from
  Bichon to `WoomyonWoods(S)` could show the correct forest map and original
  forest monsters while stale Bichon `Royal_Guard` / `Royal_Archer` rows
  remained in the packet-first entity table. Evidence: Web
  `pnpm --dir apps/web exec tsc --noEmit --pretty false` passed.
- 2026-05-21 scene backdrop edge/fallback pass: the Player Web main scene no
  longer disables its terrain fallback just because some original floor sprites
  are present. `GameSceneBackdrop` now always draws the synthetic terrain tile
  underlay behind original Crystal floor sprites, scene/UI image elements retry
  failed loads with a cache-busted same-origin URL and then the manifest remote
  asset base when available, visible scene asset preloading follows the same
  retry candidates, and scene blueprint requests prefetch a wider margin so
  chunk edges are refreshed before the player reaches the loaded play bounds.
  Evidence: `pnpm --dir apps/web exec tsc --noEmit --pretty false` passed, and
  local Web `127.0.0.1:13017` against live `wss://mir2.obelisk.build/ws`
  passed
  `docs/generated/player-qa/movement-jitter/map-scene-fallback-ui-retry-final-20260521.json`
  with `ok=true`, `sceneAssetReadiness=127/127`, no visual jumps/rollback,
  no route spam, no non-favicon 404s, and no critical console errors;
  screenshot:
  `docs/generated/player-qa/movement-jitter/map-scene-fallback-ui-retry-final-20260521.png`.
- 2026-05-21 shared NPC/monster sprite retention fix: Bichon NPC nameplates and
  minimap dots could appear while several NPC bodies were missing because a
  later shared-Zone packet refresh could lose the retained sprite image and
  re-emit `ObjectNpc` / `ObjectMonster` with `image=0`; the Web client then
  accepted that placeholder packet and replaced the correct `NPC/<image>` or
  `Monster/<image>` sprite from the world snapshot. Gateway now stores simple
  Crystal sprite snapshots when converting `ObjectNpc` / `ObjectMonster` into
  shared entities, preserves an existing sprite when merging later shared
  packets, and serializes the retained image back into shared spawn packets.
  Player Web now keeps an existing NPC/monster sprite when a packet lacks a
  sprite or carries a conflicting `image=0`, and it also falls back to the
  app-local Crystal actor sprite manifest when a live `worldSnapshot` arrives
  with NPC/monster `sprite=null`. The movement diagnostic script records compact
  entity sprite state plus rendered `.entity-sprite-stack` image load details
  for future live NPC visual checks. Evidence: focused Gateway shared object
  sprite regressions passed for `shared_zone_state_records_object_*`; Web
  typecheck and movement diagnostic syntax checks passed; local Web
  `127.0.0.1:13016` against live `wss://mir2.obelisk.build/ws` passed
  `docs/generated/player-qa/movement-jitter/npc-sprite-retention-local-live-gateway-20260521-freshdev.json`
  with 22 NPC/monster actors, `missingSprites=0`, 18 rendered sprite stacks,
  `emptyRendered=0`, no non-favicon 404s, and no critical console errors.
- 2026-05-20 production Items R2 completeness repair: Production image 404s
  for high item-icon frames such as `/original-ui/Items/2723.png` through
  `/original-ui/Items/2732.png` were traced to the live R2 release having only
  the curated `Items` export while the full Crystal `Items.Lib` has 5380
  frames. `apps/web/scripts/export-crystal-ui.mjs` now supports exporting a
  selected full library into a temporary staging root; `Items` was exported from
  `downloads/crystal-client-full/Data/Items.Lib` into `/tmp/mir2-original-ui-full`
  and uploaded to R2 prefix `mir2/v/37596e16d64fde7c` through the authenticated
  bulk upload Worker route `assets.mir2.obelisk.build/upload*`. The Cloudflare
  player-domain proxy now resolves `/api/original-ui-meta?library=...` from R2
  static `/original-ui/<library>/meta.json` when present, then falls back to
  Vercel. Evidence: production probes through the user's proxy returned 200 for
  `Items/2723-2732.png`, `Items/983.png`, `Items/984.png`, `Prguse/983.png`,
  `Monster/010/10,16,17,18,19.png`, `Monster/012/17,19.png`,
  `CArmour/00/832-835.png`, `CHair/00/16,18,19,832-835.png`,
  `CWeapon/00/17.png`, `NPC/05/11.png`, `Title/320.png`, `Title/321.png`,
  `Sound/Login2.wav`, `/api/asset-manifest`, `/api/scene/crystal?...`, and the
  Bevy wasm. Static and API `Items` metadata both report 5380 frames with
  frames 2723, 2730, and 5379 present. Production cache smoke
  `/tmp/mir2-cache-verify/items-full-r2-verify-120s.json` passed with
  `ok=true`, cold/warm `prewarmOk=387`, `prewarmFailed=0`, warm CacheStorage
  383 entries / 51.1MB, `noCriticalConsoleErrors=true`, and
  `noNonFavicon404s=true`.
- 2026-05-20 production browser-network follow-up: After the Items repair,
  Chrome/Browser runtime evidence was refreshed through login and character
  select with the default `demo/demo` account. The page asset observer recorded
  416 URLs across login, select, HUD, Bichon scene prewarm, wasm, audio, map
  tiles, and UI sprites; direct status probing found no durable Mir2 asset 404s.
  User-reported `/original-ui/ChrSel/12.png` and
  `/original-ui/Monster/010/3.png` both return 200 from the player domain, and
  the transient `Prguse/1932.png` fetch failure also returns 200 on direct
  recheck. All local `original-ui/**/meta.json` files plus generated original
  UI manifests were uploaded to the active R2 prefix so static metadata requests
  such as `/original-ui/ChrSel/meta.json` now return 200 without waiting for the
  Vercel API fallback. Production cache smoke
  `/tmp/mir2-cache-verify/chrome-network-post-rum-120s.json` passed with
  `ok=true`, `prewarmOk=387`, `prewarmFailed=0`, warm CacheStorage 383 entries
  / 51.1MB, `noCriticalConsoleErrors=true`, and `noNonFavicon404s=true`.
  `/cdn-cgi/rum?` noise is Cloudflare Analytics/RUM on a reserved Cloudflare
  path, not a Mir2 game asset route; the Mir2 asset Service Worker now responds
  to same-origin `/cdn-cgi/rum` with `204 no-store` once the player page is
  controlled by the SW so repeated DevTools sessions do not keep surfacing it as
  a missing game resource.
- 2026-05-22 live Chrome resource-error retry follow-up: The user's connected
  Chrome tab still showed preserved red `Img` rows after the production asset
  repairs, but direct player-domain probes for the current broken DOM URLs all
  returned `200 image/png`, including `generated/original-map-blend` torch
  blends, `CArmour/00/8.png`, `CHair/00/8.png`, `CWeapon/00/8.png`,
  `Monster/000/*`, `Monster/139/*`, `NPC/05/9.png`, `NPC/08/1.png`,
  `NPC/16/1.png`, `NPC/27/1.png`, `NPC/45/1.png`, `NPC/83/1.png`,
  `Prguse/983.png`, `Prguse/2044.png`, plus the user-reported
  `ChrSel/12.png` and `Monster/010/3.png`. Root cause for the remaining bad
  image state is transient first-load failure without a later retry, not durable
  missing PNGs. Scene/UI image error handling now keeps the existing same-origin
  and remote CDN fallback, schedules cache-busted delayed retries at
  0.5s/1.5s/3.5s/7s/12s after `onError`, and runs a game-scene stalled-image
  rescue pass every 1.5s so pending images that never fire `onError` are also
  cache-busted and retried. Deployment `dpl_4Uw447PEm7Y656TYHXNnHeVvnzi5` is
  READY at `https://mir2-web3-1ie43fh9n-obelisk-labs.vercel.app` and aliased
  through `https://mir2-web3-web.vercel.app` / `https://mir2.obelisk.build`.
  Verification: Web `pnpm --dir apps/web exec tsc --noEmit --pretty false`
  passed; production build/prune reduced `.vercel/output` from
  624,756,385 bytes / 80,196 files to 43,888,088 bytes / 283 files; production
  URL probes returned 200 for all sampled current broken URLs; the live
  `mir2.obelisk.build` bundle contains both `mir2DelayedRetryCount` and
  `mir2StalledRetryCount` retry markers; and after refreshing the user's
  connected Chrome game tab, the live DOM reported `brokenCount=0` for Mir2
  `/original-ui` and `/generated/original-map-blend` images.
- 2026-05-20 SoundList fallback closure: The Crystal source tree still does not
  contain exact upstream files for SoundList entries `10022 -> 22.wav`,
  `10109 -> 109.wav`, and `705 -> ZombieRevive.wav`, but the Web asset exporter
  now publishes explicit, audited fallback WAVs under the expected original
  paths so every SoundList id resolves to a playable URL. `22.wav` is copied
  from adjacent movement clip `23.wav`, `109.wav` from adjacent struck clip
  `110.wav`, and `ZombieRevive.wav` from nearby undead BoneFamiliar clip
  `64.wav`; each entry is marked with `fallback=true`,
  `exactSourceExists=false`, and a `fallbackReason` in
  `sound-index.generated.json`. Evidence: `node
  apps/web/scripts/smoke-crystal-assets.mjs` reports `exportedSoundCount=450`,
  `missingSoundCount=0`, and `failures=[]`; production player-domain probes for
  `/original-ui/Sound/22.wav`, `/original-ui/Sound/109.wav`,
  `/original-ui/Sound/ZombieRevive.wav`, and
  `/original-ui/sound-index.generated.json` all return 200 from the active R2
  prefix. Production deployment `dpl_F77Spi5brjxcRJqS6cbMqA7cChcm` is READY
  and aliased to `mir2-web3-web.vercel.app`; the Cloudflare player-domain proxy
  version `22639255-5371-4926-88b4-92fc02919ea8` appends `no-transform` to HTML
  responses, and the final production resource smoke
  `docs/generated/player-qa/cache-metrics/cache-metrics-sound-fallback-prod-final-20260520.json`
  passed with `ok=true`, `prewarmOk=387`, `prewarmFailed=0`,
  `noCriticalConsoleErrors=true`, and `noNonFavicon404s=true`.
- 2026-05-19 prewarm-latency and scene-object pruning pass: Player Web now
  treats cache prewarm as two phases. Login/select/HUD packs stay critical, but
  the Bichon scene pack is background-only, waits until the first playable frame
  plus a 20s idle window by default, and caps its sampled scene sprite frames at
  180. `/api/asset-manifest` includes the asset-cache-pack definition in the
  manifest hash, so phase/frame-cap changes rotate the runtime cache version.
  The first visible scene loader also prioritizes sprite URLs by distance from
  the current scene center, and original-map object sprites are mounted only
  when their rendered pixel bounds intersect the visible viewport margin. This
  reduces the focused Bichon first-scene visible asset set from the earlier
  217/218 range to 112/112 without changing packet-driven movement authority.
  Evidence: Web `pnpm --dir apps/web exec tsc --noEmit --pretty false`,
  `node --check apps/web/scripts/smoke-cache-metrics.mjs`,
  `node --check apps/web/scripts/capture-web-movement-jitter.mjs`, direct
  `pnpm --dir apps/web run build`, and targeted `git diff --check` passed.
  Local production Web `127.0.0.1:13015` against live
  `wss://mir2.obelisk.build/ws` passed playable cache smoke
  `docs/generated/player-qa/cache-metrics/cache-metrics-viewport-pruned-delay20-cache-local-20260519.json`
  with `ok=true`, cold first playable 11976.3ms, warm first playable 6022.1ms,
  387/387 prewarm ok, warm CacheStorage 439 entries / 65.9MB, no critical
  console errors, and no non-favicon 404s. The matching movement diagnostic
  `docs/generated/player-qa/movement-jitter/viewport-pruned-existing-settle9-local-20260519.json`
  has `ok=true`, 112/112 scene assets loaded, packet runtime
  `{"packetRefresh":58}`, no visual jumps, no logical rollback, no route spam,
  no stale prediction, no command queue warnings, no critical console errors,
  and no non-favicon 404s. Raw aborted image requests from viewport pruning are
  retained in the report as ignored non-critical `net::ERR_FAILED` entries.
  Production deployment `dpl_4YwqgqQdhA1HQQwPhFrA1KoTCpXP` is READY and aliased
  to `mir2-web3-web.vercel.app`; the Cloudflare domain manifest reports version
  `ecb5ff44ad1ad66b`, the matching `asset-cache-packs` hash, and
  `bichon-spawn` as background. Production playable cache smoke
  `docs/generated/player-qa/cache-metrics/cache-metrics-prod-viewport-pruned-delay20-cache-existing-20260519-221410.json`
  passed with `ok=true`, cold first playable 11673.5ms, warm first playable
  13549.9ms, 387/387 prewarm ok, warm CacheStorage 437 entries / 54.5MB, no
  critical console errors, and no non-favicon 404s. Production movement
  diagnostic
  `docs/generated/player-qa/movement-jitter/prod-viewport-pruned-existing-settle9-20260519-221630.json`
  passed with `ok=true`, 124/124 scene assets loaded, packet runtime
  `{"packetRefresh":58}`, no visual jumps, no logical rollback, no route spam,
  no stale prediction, no command queue warnings, no critical console errors,
  and no non-favicon 404s.
- 2026-05-19 mobile landscape controls pass: Player Web now has a first-class
  phone landscape input layer instead of relying on desktop mouse/keyboard only.
  `nipplejs` is used only as the analog joystick sensor; a Web-owned Mir2
  semantic adapter converts stick vectors into Crystal 8-way `walk` / `run`
  direction intents and sends them through the existing packet runtime/Zone
  input path. The mobile layer keeps only the latest joystick intent, gates
  sends while the packet movement runtime still has pending prediction or
  correction state, de-duplicates repeated same-direction `Turn` packets when a
  tile is blocked, and adds a mobile-MMO style right-bottom circular action
  wheel for Run, Attack, approach, pickup, inventory, character, belt items, and
  known skills. Landscape CSS exposes the controls with a scaled Crystal stage;
  portrait shows a rotate prompt. Evidence: Web
  `pnpm --dir apps/web exec tsc --noEmit --pretty false` passed,
  `node --check apps/web/scripts/capture-web-movement-jitter.mjs` passed, and
  the live mobile viewport smoke
  `mobile-controls-joystick-longhold3-20260519` passed with `ok=true`,
  `strictStatus="settled"`, no visual jumps, no logical rollback, no route spam,
  no stale prediction, no command queue warnings, no console errors, and no
  non-favicon 404s. Screenshot/report:
  `docs/generated/player-qa/movement-jitter/mobile-controls-joystick-longhold3-20260519.png`
  and `docs/generated/player-qa/movement-jitter/mobile-controls-joystick-longhold3-20260519.json`.
  The circular right-bottom wheel layout was then verified on
  `mobile-controls-wheel-short-20260519` with `ok=true`,
  `strictStatus="settled"`, no visual jumps, no logical rollback, no stale
  prediction, no command queue warnings, no console errors, and no non-favicon
  404s; screenshot/report:
  `docs/generated/player-qa/movement-jitter/mobile-controls-wheel-short-20260519.png`
  and `docs/generated/player-qa/movement-jitter/mobile-controls-wheel-short-20260519.json`.
- 2026-05-19 scene-asset-ready movement-feel gate: Player Web now treats the
  first visible game scene as part of first-playable readiness. The client
  preloads the currently visible map/entity sprite URLs, hides the scene layers
  while that first scene is pending, blocks keyboard, mouse, and mobile movement
  input until `sceneInteractionReady=true`, and records `sceneAssetsStart`,
  `sceneAssetsReady`, and deferred-input milestones in cache diagnostics.
  Movement diagnostics now waits for `sceneInteractionReady` before sending
  input and supports `--skipStartTransfer` for production Gateway routes that
  correctly reject debug teleport. Evidence: Web
  `pnpm --dir apps/web exec tsc --noEmit --pretty false`,
  `node --check apps/web/scripts/capture-web-movement-jitter.mjs`,
  `node --check apps/web/scripts/smoke-cache-metrics.mjs`, and local Web
  `127.0.0.1:13015` against live `wss://mir2.obelisk.build/ws` passed
  keyboard-sequence movement with `ok=true`, `sceneInteractionReady=true`,
  218/218 visible scene assets loaded, `packetRuntimeModes={"packetRefresh":49}`,
  no visual jumps, no logical rollback, no route spam, no console errors, and
  no non-favicon 404s. Report:
  `docs/generated/player-qa/movement-jitter/scene-ready-local-skip-transfer-20260519.json`.
  Production rerun on `https://mir2.obelisk.build` also passed after allowing
  the production prewarm queue to settle: movement report
  `docs/generated/player-qa/movement-jitter/prod-scene-ready-prewarm-wait-20260519.json`
  has `ok=true`, 217/217 visible scene assets loaded, packet runtime
  `{"packetRefresh":59}`, no visual jumps/rollback/route spam, no console
  errors, and no non-favicon 404s. The matching production playable cache smoke
  `docs/generated/player-qa/cache-metrics/cache-metrics-prod-scene-ready-20260519-1815.json`
  has `ok=true`, cold first playable 11384.1ms, warm first playable 17644.4ms,
  527/527 prewarm ok, warm CacheStorage 570 entries / 54.9MB, no critical
  console errors, and no non-favicon 404s.
- 2026-05-19 production R2 custom asset domain and edge cache: The verified R2
  release now uses `https://assets.mir2.obelisk.build/mir2/v/37596e16d64fde7c`
  as the public asset base, with `infra/cloudflare/mir2-r2-asset-cache`
  deployed on `assets.mir2.obelisk.build/*` in front of bucket
  `mir2-web3-assets`. Production `/api/asset-manifest` returns
  `remoteAssets.assetBaseUrl="https://assets.mir2.obelisk.build/mir2/v/37596e16d64fde7c"`
  and `remoteAssets.objectPrefix="mir2/v/37596e16d64fde7c"`. Repeated public
  GET probes for scene sprite frames return `x-mir2-edge-cache: HIT` and
  `cf-cache-status: HIT`, so repeat asset requests are served from Cloudflare
  edge cache instead of repeatedly fetching from R2. `/bevy-runtime/...` now
  stays same-origin with short cache headers and a build-version query on the
  JS/WASM pair, preventing R2 release prefixes from serving stale runtime files.
  Evidence: production playable smoke
  `codex-r2-assets-domain-prod-smoke-final` passed with `ok=true`, cold first
  playable 3563.4ms, warm first playable 3775.9ms, 517/517 prewarm ok, no
  critical console errors, and no non-favicon 404s. Report:
  `docs/generated/player-qa/cache-metrics/cache-metrics-codex-r2-assets-domain-prod-smoke-final.json`.
- 2026-05-19 production R2 scene sprite closure: The live R2 release at
  `mir2/v/37596e16d64fde7c` now includes the generated `/original-ui` actor,
  NPC, and Monster scene sprite roots that live gameplay requests after first
  render. The published manifest reports 7,319 asset files, 6,807 scene sprite
  files, and 0 missing files; public probes for `Monster/003/52.png`,
  `Monster/003/57.png`, `NPC/03/0.png`, `CArmour/00/12.png`,
  `AWeapon/00%20L/12.png`, and `ARWeapon/00%20S/12.png` returned 200 with
  immutable cache headers. Production playable smoke on
  `https://mir2.obelisk.build` passed with `ok=true`, cold first playable
  4296.3ms, warm first playable 4049.6ms, 517/517 prewarm ok, 0 prewarm
  failures, no critical console errors, and no non-favicon 404s. Report:
  `docs/generated/player-qa/cache-metrics/cache-metrics-codex-r2-actor-sprites-prod-smoke.json`.
- 2026-05-21 Sui wallet picker / Dubhe Wallet login pass: Player Web no longer
  auto-selects the first Sui wallet returned by Wallet Standard. The login
  dialog's `Wallet` action now opens a compact Sui wallet picker, lists all
  detected wallets that support `sui:signPersonalMessage`, prioritizes wallets
  whose id/name match Dubhe, and passes the selected wallet id into the existing
  Sui personal-message login token flow. If Dubhe Wallet is not registered in
  the browser, the picker keeps a direct `Dubhe Wallet` entry to
  `https://dubhe.obelisk.build/en/wallet`; when a Wallet Standard Dubhe wallet
  is registered, the picker shows it as the selectable wallet and hides the
  external entry. Evidence: `pnpm --dir apps/web exec tsc --noEmit --pretty false`
  passed; local Chrome/CDP smoke on `http://127.0.0.1:13010` verified the picker
  is visible, `aria-expanded=true`, the Dubhe link is present with no wallet
  installed, no critical console errors, and no overlap with the original login
  buttons. A second CDP smoke injected a standards-shaped `Dubhe Wallet` and
  verified it appeared as a selectable Dubhe-prioritized wallet with the install
  link hidden. Evidence files:
  `docs/generated/player-qa/wallet-picker/dubhe-wallet-picker-login.json`,
  `docs/generated/player-qa/wallet-picker/dubhe-wallet-picker-login.png`, and
  `docs/generated/player-qa/wallet-picker/dubhe-wallet-picker-registered.json`.
- 2026-05-18 production Vercel/Cloudflare playable smoke: `https://mir2.obelisk.build`
  now serves Player Web from Vercel project `obelisk-labs/mir2-web3-web` while
  routing `/ws` to the UCloud Gateway and `/original-map/*` to the R2 release
  prefix `mir2/v/37596e16d64fde7c`. Original scene sprite metadata now prefers
  deployed static `/original-ui/.../meta.json` for libraries already included in
  the Vercel static output, avoiding request-time `/api/original-ui-meta`
  exports on Vercel for `CHair/00`, `CWeapon/00`, `Monster/010`, `NPC/05`,
  `CArmour/00`, and `Monster/012`. Evidence:
  `npx tsc --noEmit --pretty false` passed in `apps/web`; targeted
  `git diff --check` passed; direct production probes returned 200 for those
  metadata APIs and static `original-ui`, R2 `original-map`, and Vercel blend
  assets; `npm run smoke:playable-metrics -- --baseUrl https://mir2.obelisk.build --runId prod-mir2-obelisk-final-002458 --waitTimeoutMs 300000`
  passed with `ok=true`, cold first playable 4612.5ms, warm first playable
  4684.3ms, 517/517 prewarm ok, 0 prewarm failures, no critical console errors,
  and no non-favicon 404s. Report:
  `docs/generated/player-qa/cache-metrics/cache-metrics-prod-mir2-obelisk-final-002458.json`.
- 2026-05-18 ranking-system UI pass: Player Web now opens a real Crystal-style `Ranking` social panel from the in-game System Menu instead of static placeholder rows. The panel requests Gateway `getRanking`, supports Overall, class tabs, Online, manual Refresh, selected-row details, and My Rank display, then renders typed `Rankings` payload data from the server. Evidence: Web `npx tsc --noEmit --pretty false` passed; Rust Simulation/Gateway fmt/check and focused ranking/Gateway tests passed; live Browser smoke on `http://127.0.0.1:13012/?gatewayWs=ws://127.0.0.1:7222/ws&movementDiag=1&codexBust=ranking-smoke-20260518` logged into Scout, opened Menu -> Ranking, verified Overall and Online tabs showing `Scout #1`, `Level 7 Warrior`, and `My rank: 1 / 1`; screenshot and state evidence are `docs/generated/player-qa/ranking-system/ranking-panel.png` and `docs/generated/player-qa/ranking-system/ranking-smoke.json`.
- 2026-05-18 character creation class picker pass: Player Web `NEW` now opens an original-select-screen creation panel instead of creating a hidden random male Warrior. The panel supports name entry, male/female gender selection, all five Crystal classes (`Warrior`, `Wizard`, `Taoist`, `Assassin`, `Archer`), localized Chinese labels, class icon buttons, and live select-portrait preview; `NewCharacter` now sends the selected class/gender/name and selects the newly created visible slot on success. Evidence: Web `npx tsc --noEmit --pretty false` passed in the main checkout and the currently served `/private/tmp/mir2-main-human` web directory; targeted `git diff --check` passed; Browser smoke on `http://127.0.0.1:13010/?gatewayWs=ws://127.0.0.1:7210/ws&movementDiag=1` logged into `demo/demo`, opened the localized create panel, created a female Archer visible as `QAPAPAGKA 1 弓箭手`, then cleaned that demo QA character; protocol smoke wrote `docs/generated/player-qa/create-character-classes-20260518/class-protocol-smoke.json` with `ok=true` after creating all five classes across two temporary accounts, and Browser evidence lives in `docs/generated/player-qa/create-character-classes-20260518/browser-summary.json` plus screenshots.
- 2026-05-18 Web Packet Runtime movement pass: Player Web now treats Crystal typed packets as the live in-game state source after bootstrap/reconnect/map-change/scene bootstrap. Normal `worldSnapshot` refreshes enter `packetRefresh` mode and merge only durable metadata into the packet-owned entity/drop tables, so stale snapshot rows cannot overwrite current `UserLocation`, `ObjectWalk/ObjectRun`, `ObjectNpc/ObjectMonster`, `ObjectRemove`, or live ground-drop packet state. Removed objects are tombstoned for the refresh window to prevent snapshot reinsert. The movement harness now records `worldSnapshotRealtimeMode` and `packetRuntime` mode counts, and its center-sprite jump check ignores expected residual map-scroll offset when Crystal movement direction changes. The missing `NPC/94` source-library path is also closed by making `/api/original-ui-meta` trigger the existing on-demand library export path. Evidence: `pnpm --dir apps/web exec tsc --noEmit --pretty false`, `node --check apps/web/scripts/capture-web-movement-jitter.mjs`, `curl http://127.0.0.1:13014/api/original-ui-meta?library=NPC%2F94`, `docs/generated/player-qa/movement-jitter/r-web-packet-runtime-keyseq-20260518b.json`, and `docs/generated/player-qa/movement-jitter/r-web-packet-runtime-holdspam-20260518d.json` all passed with `ok=true`, `packetRuntimeModes={"packetRefresh":...}`, no visual jumps, no logical tile rollback, no route spam, responsive movement queue, clean settle, no console errors, and no non-favicon 404s.
- 2026-05-18 Bichon click-route air-wall pass: Player Web target movement now uses a bounded local route search when the direct Crystal step is blocked by visible static map cells, visible live objects, or recent server correction memory. This keeps keyboard/directional movement unchanged while allowing right-click target movement to route around building/bridge/tree edges instead of stopping at the first non-monotonic detour. If the clicked target tile itself is blocked, the route settles on the nearest reachable tile toward the target rather than leaving a stale pending plan. Evidence: Web `npx tsc --noEmit --pretty false` passed in the main checkout and in the currently served `/private/tmp/mir2-main-human` web directory; targeted `git diff --check` passed; 13010 was restarted; Browser smoke on `http://localhost:13010/?gatewayWs=ws://127.0.0.1:7210/ws&movementDiag=1` logged into `demo/demo`, clicked through the Bichon shop/bridge area, and wrote `docs/generated/player-qa/airwall-route-20260518/airwall-route-summary.json` plus `airwall-route-after.png` with `consoleWarningsAndErrors=[]`.
- 2026-05-18 reconnect/resume grace pass: Player Web now has a committed `npm run smoke:reconnect-resume` harness for the unexpected in-game Gateway WebSocket close path, and Gateway keeps active sessions under a short reconnect grace lease instead of immediately dropping Zone presence on socket loss. The client still keeps the active auth/character slot snapshot, shows compact in-stage reconnect status, retries with bounded backoff, and replays `clientVersion`/`login`/`startGame` or a still-valid Sui token login; the backend now retains the active `GatewaySession` for `MIR2_GATEWAY_RECONNECT_GRACE_SECONDS` (default 15s, clamped 1-120s) and restores it on the next authenticated `StartGame` for the same account/character. Manual reset/logout and server `Disconnect` packets still clear reconnect state instead of looping. Evidence: Web `npx tsc --noEmit --pretty false`, `node --check apps/web/scripts/smoke-reconnect-resume.mjs`, Gateway `cargo +1.89.0 fmt --check -p mir2-gateway`, focused reconnect store tests 2/2, production Web path tests 3/3, and route-lease stale-owner regression passed. Live CDP smoke on `http://127.0.0.1:13011/?gatewayWs=ws://127.0.0.1:7211/ws` wrote `docs/generated/player-qa/reconnect/reconnect-resume-codex-reconnect-grace-smoke-final.json` with `ok=true`: it entered `demo/demo`, invoked `window.__mir2Stage5.closeGatewayForReconnectSmoke()`, observed `reconnectStatus={mode:"scheduled",attempt:1}` and `Connection lost. Reconnecting in 1s.`, then returned to `screen=game`, `wsState=open`, `reconnectStatus=idle`, same map `0`, and player `{x:336,y:249}`. Gateway logs for that run showed the grace path retained and restored session `demo/0`; the smoke records existing optional `NPC/94` original-ui meta 404s as allowed non-reconnect asset noise, with no unexpected 404s or critical console errors.
- 2026-05-18 original audio settings pass: Player Web now has persisted Music and Effects toggles backed by `mir2.originalAudioSettings`. Login and character-select screens expose compact top-right controls, and the in-game chat Settings panel exposes the same audio controls alongside channel/transparency settings. The shared audio manager now suppresses/resumes looping login/select music from the stored Music flag and suppresses Crystal `PlaySound`/button effects from the stored Effects flag. Evidence: Web `npx tsc --noEmit --pretty false` passed, targeted `git diff --check` passed, and Playwright smoke on `http://127.0.0.1:13013/?skipRuntime=1` verified the login `Audio` region, toggled `Music` and `Effects` to `Off`, confirmed `localStorage` persisted `{"musicEnabled":false,"effectsEnabled":false}`, captured a browser screenshot, and verified Simplified Chinese labels render as `声音` / `音乐` / `音效`. The only browser console error observed was the expected WebSocket refusal after an intentional `Quick Enter` attempt without a gateway running, not from the audio settings path.
- 2026-05-18 game-grade asset cache foundation: Player Web now has production immutable HTTP caching for `/original-ui`, `/original-map`, and `/bevy-runtime`; a versioned `/api/asset-manifest`; opt-in-dev/auto-production `mir2-asset-worker.js` runtime caching for static game assets, scene blueprints, and metadata; and a server-side `.next/cache/mir2-scene-blueprints` memory/disk cache for `GET /api/scene/crystal`. Evidence: Web `npx tsc --noEmit --pretty false`, `node --check apps/web/public/mir2-asset-worker.js`, `git diff --check` on changed cache files, and direct `npx next build` all passed; dev server `http://127.0.0.1:13011` scene probe for `map=0&x=420&y=257&width=32&height=28` returned first-run `X-Mir2-Scene-Cache=miss` in 124ms and second-run `hit` in 10ms; production `next start` on `http://127.0.0.1:13012` served `/original-ui/Prguse/4.png` with `Cache-Control: public, max-age=31536000, immutable`, served `/api/asset-manifest` with `max-age=60, stale-while-revalidate=300`, served `/mir2-asset-worker.js` with `Service-Worker-Allowed: /`, and returned production scene `Cache-Control: public, max-age=300, stale-while-revalidate=3600` with cache hits at 42ms/9ms. Browser smoke on `http://127.0.0.1:13011/?assetCache=1&skipRuntime=1` loaded the login shell with `critical console errors=[]`; the Browser plugin's read-only page context did not expose Cache Storage internals, so SW internals were verified by route/header/build behavior rather than direct Cache API enumeration.
- 2026-05-18 cache metrics and critical prewarm: Player Web now exposes QA-only cache observability through `?cacheDebug=1` and `window.__mir2CacheMetrics`, with resource timing, transfer/encoded bytes, cache-like resource counts, scene cache hit/miss counts, slowest resource samples, and prewarm status. `/api/asset-manifest` now includes critical prewarm packs for `login`, `character-select`, `hud-core`, and `bichon-spawn`; the Bichon pack fetches the scene blueprint and first visible scene sprite frames instead of preloading the full Crystal source tree. Evidence: `curl http://127.0.0.1:13011/api/asset-manifest` returned 4 resource packs with 40 login URLs, 41 character-select URLs, 108 HUD URLs, and 1 Bichon scene prewarm; Browser smoke on `http://127.0.0.1:13011/?assetCache=1&cacheDebug=1&skipRuntime=1` showed the `Mir2 Cache Debug` panel, `SW: registered`, scene cache hits, and final `Prewarm: 511/511 ok, 0 failed`, with console `errors=[]`; Web `npx tsc --noEmit --pretty false` and targeted `git diff --check` passed after the metrics/prewarm implementation.
- 2026-05-18 cache metrics cold/warm smoke: Web now has repeatable `npm run smoke:cache-metrics`, which launches a fresh Chrome profile, enables `assetCache/cacheDebug/prewarm`, waits for `window.__mir2CacheMetrics` prewarm completion, runs a second warm pass in the same profile, and writes `docs/generated/player-qa/cache-metrics/latest-cache-metrics.json`. Evidence: `MIR2_WEB_BASE_URL=http://127.0.0.1:13011 npm run smoke:cache-metrics -- --runId codex-cache-smoke-final` passed with `ok=true`; cold recorded 524 game resources, 2,669,943 transfer bytes, 503 cache-like resources, 2/2 scene hits, and 503/511 prewarm ok; warm recorded 524 game resources, 0 transfer bytes, 524 cache-like resources, 2/2 scene hits, and 511/511 prewarm ok; assertions `warmCompletedPrewarm`, `noPrewarmFailures`, `noCriticalConsoleErrors`, and `noNonFavicon404s` were all true. Follow-up verification passed `node --check apps/web/scripts/smoke-cache-metrics.mjs`, Web `npx tsc --noEmit --pretty false`, targeted `git diff --check`, and direct `npx next build`.
- 2026-05-18 real first-playable cache smoke: Cache metrics now include milestones for HTML ready, Bevy/runtime decision, scene blueprint, scene sprite readiness, Gateway connect/login/select, `StartGame`, `UserInformation`, game screen readiness, and `firstPlayableFrame`. `npm run smoke:playable-metrics` drives a fresh Chrome profile through `demo/demo` login, character select, real Gateway `StartGame`, Bichon scene load, first playable frame, complete prewarm, then repeats a warm pass in the same profile. Evidence: `MIR2_WEB_BASE_URL=http://127.0.0.1:13011 MIR2_GATEWAY_WS_URL=ws://127.0.0.1:7210/ws npm run smoke:playable-metrics -- --runId codex-playable-smoke-final --waitTimeoutMs 90000` passed with `ok=true` and wrote `docs/generated/player-qa/cache-metrics/cache-metrics-codex-playable-smoke-final.json`; cold first playable was 1503.7ms with 839 game resources, 3,395,535 transfer bytes, 697 cache-like resources, 3/3 scene hits, and 511/511 prewarm ok; warm first playable was 1193.8ms with 740 game resources, 300 transfer bytes, 738 cache-like resources, 3/3 scene hits, and 511/511 prewarm ok. Assertions for first-playable presence/budgets, prewarm completion, no prewarm failures, no critical console errors, and no non-favicon 404s were all true.
- 2026-05-18 CacheStorage/quota diagnostics: `window.__mir2CacheMetrics.snapshot()` now includes Mir2 CacheStorage cache counts, entry counts, and `navigator.storage.estimate()` usage/quota values, and the QA overlay shows those values without adding player-facing UI. The smoke harness now asserts that the warm pass has populated Mir2 CacheStorage entries. Evidence: `MIR2_WEB_BASE_URL=http://127.0.0.1:13011 npm run smoke:cache-metrics -- --runId codex-cache-storage-smoke-final --waitTimeoutMs 90000` passed with warm `cacheStorageCacheCount=2`, `cacheStorageEntryCount=510`, `storageUsageBytes=65338772`, 0 transfer bytes, 511/511 prewarm ok, and all assertions true; `MIR2_WEB_BASE_URL=http://127.0.0.1:13011 MIR2_GATEWAY_WS_URL=ws://127.0.0.1:7210/ws npm run smoke:playable-metrics -- --runId codex-playable-storage-smoke-final --waitTimeoutMs 90000` passed with cold first playable 1659.6ms, warm first playable 2224.9ms, warm `cacheStorageCacheCount=2`, `cacheStorageEntryCount=555`, `storageUsageBytes=67045268`, 0 transfer bytes, 511/511 prewarm ok, no prewarm failures, no critical console errors, and no non-favicon 404s.
- 2026-05-18 Service Worker maintenance and QA reset: Player Web now exposes a QA-only `window.__mir2AssetCacheReset({ reload?: false })` helper that deletes all `mir2-asset-cache-*` CacheStorage buckets, unregisters the Mir2 asset Service Worker, marks the cache state as reset, refreshes cache metrics, and reloads by default unless `reload:false` is passed. The Service Worker also handles maintenance messages for explicit reset/status and reports stale-cache cleanup after each manifest config. Evidence: after restarting the stale 13011 dev server with current code, `MIR2_WEB_BASE_URL=http://127.0.0.1:13011 npm run smoke:cache-maintenance -- --runId codex-cache-maintenance-smoke-final --waitTimeoutMs 90000` passed with `ok=true`; warm cache had 2 Mir2 caches / 510 entries / 65335069 usage bytes / 0 transfer bytes / 511/511 prewarm ok; the maintenance pass seeded `mir2-asset-cache-static-legacy-smoke`, verified manifest-version cleanup removed it, then `__mir2AssetCacheReset({ reload:false })` deleted 3 active caches, unregistered 1 Service Worker scope, and left `afterReset.cacheNames=[]`. All maintenance assertions (`maintenanceLegacyCacheSeeded`, `maintenanceLegacyCacheCleanedByVersion`, `maintenanceManualResetAvailable`, `maintenanceManualResetClearedCaches`) were true.
- 2026-05-18 cache persistence and budget guardrails: The cache metrics now record `navigator.storage.persisted()` and the result of the one-time `navigator.storage.persist()` request so QA can see whether a browser is likely to evict cached game assets. The Service Worker no longer writes `bootstrap` runtime caches before receiving the versioned manifest, and the frontend waits for the SW configured/cleanup ACK before refreshing storage metrics. The cache smoke now enforces anti-footgun budgets: prewarm requests <= 1000, warm CacheStorage entries <= 2500, and warm browser storage usage <= 256 MiB by default. Evidence: `MIR2_WEB_BASE_URL=http://127.0.0.1:13011 npm run smoke:cache-maintenance -- --runId codex-cache-budget-maintenance-smoke-final --waitTimeoutMs 90000` passed with `ok=true`; warm pass recorded 511/511 prewarm ok, 118 warm CacheStorage entries, 62272086 storage usage bytes, `storagePersisted=false`, `storagePersistGranted=false` in fresh headless Chrome, and budget assertions `prewarmWithinBudget`, `warmCacheStorageEntriesWithinBudget`, and `warmStorageUsageWithinBudget` all true. The maintenance pass seeded the legacy cache, version-cleaned it, deleted 3 active caches, unregistered 1 SW scope, and ended with zero Mir2 caches.
- 2026-05-18 R2/CDN remote asset release pass: Player Web now supports versioned remote static asset sourcing for the existing game cache. `/api/asset-manifest` exposes `remoteAssets` with `{version}`-resolved CDN base URL/object prefix, the asset Service Worker maps same-origin `/original-ui`, `/original-map`, and `/bevy-runtime` misses to the configured remote base before falling back to app origin, and the R2 release scripts stage/upload the exact manifest-declared critical packs instead of the full 7-8 GB source tree. Evidence: current-code Web on `127.0.0.1:13014` returned `remoteAssets.objectPrefix="mir2/v/37596e16d64fde7c"`, and with `MIR2_ASSET_BASE_URL=https://assets.example.com/mir2/v/{version}` it returned `remoteAssets.enabled=true` plus `assetBaseUrl="https://assets.example.com/mir2/v/37596e16d64fde7c"`; `npm run assets:remote:build -- --baseUrl http://127.0.0.1:13014 --assetBaseUrl https://assets.example.com/mir2/v/{version} --runId codex-r2-release-smoke` wrote `docs/generated/remote-assets/codex-r2-release-smoke/remote-asset-release.json` and `latest-remote-asset-release.json` with `stats.fileCount=512`, `stats.totalBytes=64626176`, `stats.missingCount=0`; `npm run assets:r2:dry-run -- --manifest docs/generated/remote-assets/codex-r2-release-smoke/remote-asset-release.json` reported `uploadCount=513`, `totalBytes=65000146`, and sample object keys under `mir2/v/37596e16d64fde7c/`. Live R2 upload is verified: bucket `mir2-web3-assets` has 513/513 objects under `mir2/v/37596e16d64fde7c`, public access is enabled at `https://pub-72ec6e670a8346d1a6b2177df2643326.r2.dev`, GET/HEAD CORS allows `*`, public `original-ui/Prguse/4.png` returns 200 with immutable cache headers, and public `remote-asset-release.json` reports `assetBaseUrl="https://pub-72ec6e670a8346d1a6b2177df2643326.r2.dev/mir2/v/37596e16d64fde7c"`, `stats.fileCount=512`, `stats.missingCount=0`. `node --check` passed for both new scripts and `mir2-asset-worker.js`.
- 2026-05-17 item/equipment hover info pass: Player Web now renders in-game Crystal-style item info tooltips for inventory, storage, belt, and character equipment slots instead of relying on native browser `title` text or showing no panel. Tooltips use the live snapshot fields already provided by Gateway (`name`, `description`, stack quantity, durability, attack, defence) and are anchored per slot to avoid right-edge overflow. Evidence: Web `npx tsc --noEmit --pretty false` passed in both the main checkout and the currently served `/private/tmp/mir2-main-human` web directory; 13010 was restarted so the updated tooltip CSS is served; Browser DOM inspection after `demo/demo` game entry confirmed visible belt items include `.original-item-tooltip` content such as `Red Potion`, its description, and `Quantity`; Browser console error check returned `[]`. Browser hover synthesis did not set `:hover` in the in-app automation surface, so final pixel/feel acceptance remains manual.
- 2026-05-17 login Web3 action integration: Player Web now places the Passkey/Wallet alternatives inside the original login dialog's dark credential well instead of floating them below the panel, using compact gold-brown button treatment that reads as a secondary login action alongside ID/PASS. Evidence: Web `npx tsc --noEmit --pretty false` passed in both the main checkout and the currently served `/private/tmp/mir2-main-human` web directory; the 13010 Web dev server was restarted so the updated CSS was served; Browser screenshot inspection on `http://localhost:13010/?gatewayWs=ws://127.0.0.1:7210/ws` confirmed the actions sit inside the login dialog with no text overlap; Browser console error check returned `[]`.
- 2026-05-17 login error-state hardening: Player Web now treats Gateway `type="error"` messages during login like terminal login failures, clearing pending password/new-account/Sui-login refs, ending the login busy overlay, and surfacing the Gateway error on the login panel instead of leaving the client stuck on `Logging in...` / `正在登录...`. The same close-path cleanup remains in place for disconnected sockets. Evidence: Web `npx tsc --noEmit --pretty false` passed in both the main checkout and the currently served `/private/tmp/mir2-main-human` web directory; an initial live Gateway WS probe confirmed the previous running binary emitted a `type="error"` response for unsupported `passkeyLogin`; a Browser login smoke on `http://localhost:13010/?gatewayWs=ws://127.0.0.1:7210/ws` reached character select with `demo/demo`; and after refreshing the running Gateway with the coherent temp build, a live HMAC-token WS probe confirmed `passkeyLogin` returns `LoginSuccess`. The main source checkout still has broader Gateway/Simulation WIP to reconcile before rebuilding cleanly from main source.
- 2026-05-16 strict Crystal CurrentLocation movement sync: Player Web now commits each local self Walk/Run action target into the local `WorldEntity` at action start, matching Crystal `PlayerObject.SetAction()` where `CurrentLocation` is advanced before `OffSetMove` draws the sprite back from the source tile. Self `UserLocation` packets and periodic `worldSnapshot` self entries are now allowed to confirm/stale-echo the active local action without overwriting that local CurrentLocation; only true corrections can hard-reset the self transform. Evidence: Web `npx tsc --noEmit`, `node --check apps/web/scripts/capture-web-movement-jitter.mjs`, high-frequency Shift+`D/A` capture `docs/generated/player-qa/movement-jitter/r-strict-actionfeed-current-location-230738.json`, 10s Shift+`D` capture `docs/generated/player-qa/movement-jitter/r-strict-actionfeed-current-location-long-230835.json`, and held-run-plus-spam-click capture `docs/generated/player-qa/movement-jitter/r-strict-actionfeed-current-location-clickspam-231926.json`, all with `ok=true`, `noVisualJumps=true`, `noLogicalTileRollback=true`, `movementCommandQueueResponsive=true`, clean settle, `pendingPlanAtEnd=null`, no console errors, and no non-favicon 404s.
- 2026-05-16 all-map resource/gameplay audit closure: Web map coverage and runtime map semantics now treat Crystal empty/out-of-range map sprite frames as Crystal `MLibrary.GetSize/Draw` no-draw behavior instead of frontend fallback risk. `audit:crystal-map-coverage` records 463/463 maps present and parseable, unsupported map types 0, parse errors 0, missing minimap indices `[]`, missing sampled map libraries 0, `visualFallbackRisk.mapCount=0`, and 453 Crystal-ignored no-draw frame references tracked separately. The new `audit:crystal-map-gameplay` records 1999 movement rows checked with 1906 direct transfers, 93 Crystal-ignored/deferred/special transfers, movement failures 0, 6341 respawn rows with 6293 candidate-backed and 48 Crystal-inert no-candidate warnings, respawn failures 0, 375 NPC rows with scripts found, 7 empty placeholder warnings, unimplemented NPC commands 0, and static map semantic failures 0 across safe zones, safe-zone spell flags, doors, cell lights, fishing cells, drop rules, and light/feature flags. Runtime fixes in Simulation also make local full-client map lookup work on this checkout, correct type-1 map cell stride parsing, suppress invalid/special Crystal movement transfers from runtime `transfer_map`, and avoid spawning monsters at invalid origins when Crystal would leave a respawn inert. Evidence: `CRYSTAL_CLIENT_ROOT=/Users/henryliu/obelisk/ai/numeron/mir2/downloads/crystal-client-full node apps/web/scripts/audit-crystal-map-coverage.mjs`, `CRYSTAL_CLIENT_ROOT=/Users/henryliu/obelisk/ai/numeron/mir2/downloads/crystal-client-full node apps/web/scripts/audit-crystal-map-gameplay.mjs`, Web `npx tsc --noEmit`, `cargo +1.89.0 fmt --check -p mir2-simulation`, focused Simulation `crystal_manifest_movements` 2/2, and focused Simulation `spread_slots` 2/2.
- 2026-05-15 local CurrentLocation movement closure: Player Web now promotes visually completed self Walk/Run actions into the local self `WorldEntity` / `sceneView.center`, matching Crystal's local `CurrentLocation` update instead of leaving the completed tile in a long-lived predicted/anchor layer while waiting for delayed snapshots. Stale `worldSnapshot` self positions are ignored when the local self transform is still a plausible forward Crystal action, so old `UserLocation` / snapshot echoes cannot pull the rendered player back after high-frequency input. Evidence: Web `npx tsc --noEmit`, direct `npx next build`, `node --check apps/web/scripts/capture-web-movement-jitter.mjs`, `docs/generated/player-qa/movement-jitter/r-highfreq-keyseq-da-after-local-current-location-16ms.json`, `docs/generated/player-qa/movement-jitter/r-long-shiftd-after-local-current-location-16ms.json`, and `docs/generated/player-qa/movement-jitter/r-right-left-after-local-current-location-16ms.json` all record `ok=true`, `noVisualJumps=true`, `noLogicalTileRollback=true`, `movementCommandQueueResponsive=true`, `movementSettledWithoutResidualPlan=true`, `pendingPlanAtEnd=null`, no console errors, and no non-favicon 404s.
- 2026-05-15 high-frequency keyboard movement closure: Player Web now keeps a separate service-confirmation ledger for self Walk/Run actions, distinct from the visual ActionFeed. When WASD/Arrow input reverses direction while older movement commands are still waiting for `UserLocation`, the client updates the latest intent but does not derive a new opposite-direction command from stale speculative tiles. This prevents old server confirmations from pulling the rendered player backward during high-frequency run/turn input. The movement harness now supports strict `keyboardSequence` captures, pre-input warmup, WebSocket movement frame tails, and direction-step source-aware route-spam classification. Evidence: `docs/generated/player-qa/movement-jitter/r-highfreq-keyseq-da-after-outstanding-gate-170756.json` records 36 rapid Shift+`D/A` taps with `ok=true`, `jumps=[]`, `logicalRollbackWarnings=[]`, `commandQueueWarnings=[]`, `pendingPlanAtEnd=null`, `outstandingSelfMovementActions=[]`, no console errors, and no non-favicon 404s; `docs/generated/player-qa/movement-jitter/r-highfreq-right-then-left-after-outstanding-gate-170859.json` records a right-run then left-run reversal with the same zero-warning result and final `direction=Left`; `docs/generated/player-qa/movement-jitter/r-long-shiftd-after-outstanding-gate-170946.json` keeps the 12s Shift+`D` long-run regression green.
- 2026-05-15 long continuous run/walk rollback closure: Player Web now treats repeated same-tile `UserLocation` confirmations during held direction input as a Crystal-style blocked action instead of letting the client keep sending stale Walk/Run intents into the blocked tile. The movement input loop records no-progress self acks, marks both walk/run blocked at the authoritative source tile, suppresses the held direction for the route-block memory window, clears local action/render anchors on true correction, and keeps the separate local render lead window at two tiles while action feed lookahead can remain wider for delayed Zone confirmations. Evidence: `docs/generated/player-qa/movement-jitter/r-long-shiftd-fresh-after-first-block.json` records a 12s Shift+`D` run from `{330,270}` to `{345,270}` with `ok=true`, `noVisualJumps=true`, `noLogicalTileRollback=true`, `noRouteSpamWarnings=true`, `movementCommandQueueResponsive=true`, `movementSettledWithoutResidualPlan=true`, no console errors, and no non-favicon 404s; screenshot `r-long-shiftd-fresh-after-first-block.png`. Verification also passed Web `npx tsc --noEmit`, direct `npx next build`, focused Simulation `continuous_run_extends_run_grace_after_successful_run`, and `cargo +1.89.0 fmt --check -p mir2-simulation`.
- 2026-05-15 Crystal ActionFeed movement semantic alignment: Player Web now keeps a local self-action feed matching Crystal's `QueuedAction` / `ActionFeed` split. Local Walk/Run actions record source tile, target tile, direction, mode, sent time, and visual window; self `UserLocation` packets are first classified as confirmed action, stale echo, partial run confirmation, or true correction before any hard rollback is allowed. Rendering and debug `state.player` now fall back to the latest local self action target while authoritative packets catch up, so held keyboard/run movement does not snap back to an older server tile during normal confirmation lag. The Bevy runtime boot path also supports `skipRuntime=1` for DOM-only movement harnesses and avoids duplicate same-page boot after HMR. Evidence: Web `npx tsc --noEmit`, direct `npx next build`, `node --check apps/web/scripts/capture-web-movement-jitter.mjs`, and the CDP mini smoke `docs/generated/player-qa/movement-jitter/r-crystal-actionfeed-mini-smoke3.json` record Shift+`D` from `{330,270}` to `{345,270}` with `ok=true`, `rollbackCount=0`, `staleSampleCount=0`, final `feed=[]`, final `queue=[]`, plus screenshot `r-crystal-actionfeed-mini-smoke3.png`.
- 2026-05-15 Crystal chat control-bar closure: Player Web now treats the `Prguse/2034` chat control row like Crystal `ChatControlBar` instead of using it as display-only filters. All/Shout/Whisper/Lover/Mentor/Group/Guild buttons set the outgoing chat prefix (`""`, `!`, `/`, `:)`, `!#`, `!!`, `!~`), preserve/reset the input after send, and no longer hide the feed themselves; the Settings panel now owns Crystal-style channel visibility filters plus transparency; Trade sends the real `tradeRequest` browser command; Size collapses/expands chat; Report opens/closes the report dialog. The row's sprite buttons now have real 24x13 hit boxes and sit above the HUD, closing the prior "looks clickable but nothing happens" class. Evidence: `MIR2_STAGE5_ACCOUNT_MODE=demo MIR2_STAGE5_SMOKE_CHAT_ONLY=1 MIR2_WEB_BASE_URL=http://127.0.0.1:13010/?gatewayWs=ws://127.0.0.1:7210/ws node apps/web/scripts/smoke-stage5-ui.mjs` wrote `docs/stage5-screenshots/stage5-chat-controls-smoke-manifest.json` with `mode="chat-controls-only"`, 13 screenshots, every chat-control hit test `topMatches=true`, verified prefixes for all seven channel buttons, `lastCommand.type="chat"` with `!Codex shout smoke`, `lastCommand.type="tradeRequest"`, settings normal-filter and transparency toggles, collapse/expand, report open/close, and `criticalConsoleErrors=[]`. Verification also passed Web `node --check apps/web/scripts/smoke-stage5-ui.mjs`, `npx tsc --noEmit`, and direct `npx next build`.
- 2026-05-14 Crystal asset pipeline productionization: Player Web now has explicit asset scripts for the full Crystal client instead of relying on ad hoc exported folders. `npm run generate:crystal-asset-index` now fails fast when the full client root is incomplete, `npm run export:crystal-sounds` parses Crystal `Sound/SoundList.lst`, copies all available referenced wavs into `public/original-ui/Sound`, and writes `public/original-ui/sound-index.generated.json` plus `docs/generated/assets/latest-sound-assets.json`. Web runtime now consumes `ServerPacket::PlaySound` through `original-audio.ts` / `original-sound-index.ts`, shares one audio manager for login/select music and effects, and plays the Crystal button effect from shared sprite buttons. The on-demand sprite API now validates against `source-libraries.generated.json`, rate-limits very large libraries by frame count, returns typed status codes (`unsupported_library`, `library_not_indexed`, `library_too_large`, `crystal_data_missing`, `library_missing`), and keeps production builds warning-free under Turbopack. Evidence: `npm run smoke:crystal-assets` records 1,440 libraries, 2,143,132 frames, 1,624 maps, 1,607 source sounds, 450 SoundList entries, 447 available sound mappings, and no failures at `docs/generated/assets/latest-crystal-asset-pipeline-smoke.json`; live Next API smoke returned 200 for source-only `NPC/223` on-demand export, 200 for `sound-index.generated.json`, 200 for `Sound/100.wav`, and 400 for unsupported `Map/*`; Web `npx tsc --noEmit`, direct `npx next build`, `smoke:crystal-minimap-assets`, and `audit:crystal-map-coverage` passed. The three missing SoundList wavs (`22.wav`, `109.wav`, `ZombieRevive.wav`) are absent from the Crystal source and recorded as non-blocking missing references.
- 2026-05-14 full Crystal client resource sync: downloaded the MirFiles Crystal patch manifest into `downloads/crystal-client-full` from `https://ftp.mirfiles.co.uk/resources/mir2/crystal/patch/`, verified all 4,698 manifest entries with 0 missing files and 0 size mismatches, and refreshed Web original UI assets from that full client root. A full asset index now records 1,440 `.Lib` libraries, 2,143,132 source frames, 1,624 map files, and 1,607 sound files at `docs/generated/assets/full-crystal-client-index.json`; `public/original-ui/source-libraries.generated.json` lets Player Web treat every non-map Crystal sprite library as available and convert a missing library on demand through `/api/original-ui-meta`. The on-demand path was verified with previously unexported `AArmour/02`, generating 1,024 frames plus `meta.json`. `Data/mmap.Lib` still stops at index 449, so minimap indices 450/451 were sourced from the Crystal database preview BMPs and exported as `public/original-ui/MMap/450.png` and `451.png`. Evidence: `docs/generated/assets/latest-minimap-assets.json` now records `missingMiniMapIndices=[]`, and `docs/generated/map/latest-crystal-map-coverage.json` records `miniMapCoverage.missingMiniMapIndices=[]` with 463/463 source maps still present/parseable.
- 2026-05-13 shared Zone live two-client browser smoke: Player Web was run against a temporary Gateway/account-store on `127.0.0.1:7210` / `127.0.0.1:13010` with two independent browser pages, then the flow was committed as repeatable `npm run smoke:two-client-zone`. Account `zonea20260513053629` / character `ZA053629` and account `zoneb20260513053629` / character `ZB053629` both reached the game screen on Crystal map `0`; after Zone placement and ticks, page A saw B as a `player` entity, page B saw A as a `player` entity, and page B received A's movement broadcast (`ObjectWalk` / `ObjectRun` evidence in WebSocket frames). Evidence: `docs/generated/player-qa/two-client-zone/two-client-zone-20260513053629.json` records `ok=true`, `bothGame=true`, `aSeesB=true`, `bSeesA=true`, `bSawMovementBroadcast=true`, `noConsoleErrors=true`, and `noNonFavicon404s=true`, with screenshots `two-client-zone-20260513053629-a.png` and `two-client-zone-20260513053629-b.png`. The repeatable script run `docs/generated/player-qa/two-client-zone/two-client-zone-script-135930.json` also records `ok=true`, `aSawChatBroadcast=true`, no console errors, and no non-favicon 404s, with screenshots `two-client-zone-script-135930-a.png` and `two-client-zone-script-135930-b.png`. The lower-level WebSocket two-client smoke `docs/generated/load/two-client-zone-smoke-133316.json` records 2/2 ready clients, 0 errors, 38 commands sent, and 1,241 received messages.
- 2026-05-12 Crystal movement rollback/cadence follow-up: Player Web no longer starts the next movement cooldown from `UserLocation` receive time; direction and target-route movement now keep the Crystal 600ms cadence anchored to the command/action window, with only a short confirm tick after authoritative packets. Recent attack/skill actions now block local movement prediction while still allowing movement packets through, and route handoff no longer preserves a future predicted tile during that combat action window. This closes the repro where repeated right-clicks on/near `Training Dummy` mixed attack packets with movement, causing the client to render `{331,270}` or `{333,270}` before the server was allowed to move and then snap back. Evidence: `docs/generated/player-qa/movement-jitter/r-crystal-input-stress-route-preserve-block-102429.json` records held right-run plus eight repeated target clicks with interleaved `attack` commands, `ok=true`, `logicalRollbackWarnings=[]`, `jumps=[]`, `stalePredictionWarnings=[]`, `commandQueueWarnings=[]`, and `holdThenSpamClickTargetQueueStrict pass=true`; `r-crystal-input-keyboard-final-102458.json` records held Shift+`D`/run with `ok=true`, four movement commands, final player `{337,270}`, no rollback, no stale prediction, no queue warnings, no browser console errors, and no non-favicon 404s. Verification passed Web `pnpm --dir apps/web exec tsc --noEmit`, live local Gateway/Web captures, and screenshot inspection at `r-crystal-input-stress-route-preserve-block-102429.png` plus `r-crystal-input-keyboard-final-102458.png`.
- 2026-05-11 Crystal input-loop hard-standard follow-up: Player Web now treats Crystal's local action display as the source of truth while server confirmations are still in flight. Same-source `UserLocation`/snapshot echoes inside the action window no longer hard-correct the client, blocked-step hints are de-duplicated before reroute, blocked run predictions cannot be restored as the same pending run, and accepted self movement packets bridge the React state frame so the visible player does not momentarily fall back to the previous server tile. Direction pending now releases after Crystal's 600ms action window plus 400ms correction window once the local predicted tile is visible, keeping WASD/Arrow held movement responsive without queue residue. Evidence: `docs/generated/player-qa/movement-jitter/r-crystal-input-align-monotonic-182603.json` records held-run plus repeated right-click target stress with `ok=true`, `jumps=[]`, `logicalRollbackWarnings=[]`, `stalePredictionWarnings=[]`, `commandQueueWarnings=[]`, and `holdThenSpamClickTargetQueueStrict pass=true`; `r-crystal-input-align-keyboard-d-release-182950.json` records held Shift+`D`/run with four `run Right` commands, no rollback, no stale prediction, and no queue warning; `r-crystal-input-align-keyboard-arrow-183040.json` records held `ArrowRight`/run with the same zero-warning movement feel. Verification passed Web `pnpm --dir apps/web exec tsc --noEmit`, live local Gateway/Web captures, and screenshot inspection at `r-crystal-input-align-monotonic-182603.png`.
- 2026-05-11 movement input-latency/tick-cadence follow-up: Player Web now starts direction-step and click-target prediction on the next Crystal tile in the same input frame instead of only turning in place while waiting for server `UserLocation`. Gateway tick handling now avoids 100ms movement-time flooding: idle ticks stay slow, and movement commands schedule one 320ms confirmation tick to unlock the next queued step without letting tick traffic swallow player movement confirms. Evidence: `docs/generated/player-qa/movement-jitter/r-input-latency-keyboard-d-0511i.json` records keyboard `D` first prediction at 52ms (`330,270 -> predicted 331,270`), first `UserLocation` at 374ms, and final `{333,270}`; `r-input-latency-shift-run-0511j.json` records Shift+`D` first prediction at 58ms to `{332,270}`, first `UserLocation` at 402ms, and final `{336,270}`; `r-input-latency-click-target-0511k.json` records click-target arrival at `{333,270}` with first prediction at capture start and first `UserLocation` at 307ms. All three record `ok=true`, `logicalRollbackWarnings=[]`, `directionLagWarnings=[]`, `stalePredictionWarnings=[]`, `commandQueueWarnings=[]`, `pendingPlanAtEnd=null`, `consoleErrors=[]`, and `nonFaviconNetwork404s=[]`. Verification passed Web `pnpm --dir apps/web exec tsc --noEmit`, `node --check apps/web/scripts/capture-web-movement-jitter.mjs`, live local Gateway/Web captures, and screenshot inspection.
- 2026-05-11 keyboard movement input follow-up: Player Web now accepts WASD and Arrow-key movement in the game stage while preserving Crystal's existing click movement path. Held keyboard directions reuse the same Crystal direction-step queue as mouse-hold input, support diagonal combinations, ignore editable chat/login/panel fields, and use Shift as run intent by targeting the two-tile Crystal run step. The old selected-target approach shortcut moved from `A` to `F` so `A` can be left movement; Space/Enter remain primary target action. Evidence: `docs/generated/player-qa/movement-jitter/r-keyboard-wasd-0511k.json` records keyboard `D` walk `330,270 -> 332,270`, `r-keyboard-arrow-0511l.json` records `ArrowRight` walk with no rollback or queue residue, and `r-keyboard-shift-run-0511m.json` records Shift+`D` as `run Right` ending at `332,270`; all three have `ok=true`, `logicalRollbackWarnings=[]`, `directionLagWarnings=[]`, `commandQueueWarnings=[]`, `pendingPlanAtEnd=null`, `consoleErrors=[]`, and `nonFaviconNetwork404s=[]`. Verification passed Web `pnpm --dir apps/web exec tsc --noEmit`, movement harness syntax check, live local Gateway/Web keyboard captures, and screenshot inspection.
- 2026-05-11 Crystal input/NPC marker hard-standard follow-up: Player Web now recovers stale in-flight movement actions before the 1200ms responsiveness threshold by re-anchoring the target plan to the authoritative server tile, clearing pending state, and retrying after a short Crystal correction delay instead of letting held-run plus repeated target clicks age into a stalled queue. The renderer also keeps unconfirmed target/direction pending tiles out of the visible player position while still reflecting immediate facing, so automated checks distinguish true visual rollback from debug queue state. Evidence: `docs/generated/player-qa/movement-jitter/r-click-target-crystal-input-final-090309.json`, `r-route-spam-obstacle-crystal-input-final-090355.json`, `r-blocked-target-crystal-input-final-090443.json`, and `r-input-queue-held-run-spam-click-crystal-input-final-090527.json` all record `ok=true`, `jumps=[]`, `logicalRollbackWarnings=[]`, `directionLagWarnings=[]`, `stalePredictionWarnings=[]`, `commandQueueWarnings=[]`, `pendingPlanAtEnd=null`, `consoleErrors=[]`, and `nonFaviconNetwork404s=[]`; the held-run stress path ends settled at `{335,270}` with `movementCommandQueueResponsive pass=true`. A temporary isolated Gateway/account-store marker fixture verified NPC click and marker geometry without touching the main local account data: `docs/generated/player-qa/npc-click/r-npc-click-marker-crystal-anchor-final-090830.json` records `dialogTitle=MirGuide_Peter`, `interactCount=1`, `moveCount=0`, `crystalLeftDeltaPx=0`, `crystalTopDeltaPx=0`, no browser errors, and no non-favicon 404s. The marker anchor follows Crystal `Client/MirObjects/NPCObject.cs` draw math: `DrawLocation + BodyLibrary.GetOffSet(BaseIndex) + (size.Width / 2 - 28, -40)`, so the icon's right edge aligns to the NPC body center as in Crystal. Verification passed Web `pnpm --dir apps/web exec tsc --noEmit`, movement/NPC script syntax checks, four live local Gateway/Web movement captures, isolated marker capture, and screenshot inspection.
- 2026-05-11 movement feel rollback follow-up: `capture-web-movement-jitter.mjs` now records and fails on logical tile rollback during an active Crystal movement direction, and it asserts `walk`/`run`/`turn` direction commands are reflected by the predicted/self sprite direction within a 260ms Crystal input-loop window. The live repro `docs/generated/player-qa/movement-jitter/r-direction-lag-logical-rollback-0511b.json` caught the old failure (`noVisualJumps=false`, `noLogicalTileRollback=false`, `logicalRollbackCount=1`) when a held run plus repeated target clicks cleared prediction from `{338,270}` back to confirmed `{334,270}` before the server caught up. Player Web now keeps the local predicted anchor through the server-lag window, does not count already-confirmed same-tile prediction as pending, and avoids converting a still-in-flight sent source into a hard route correction. Fixed evidence: `docs/generated/player-qa/movement-jitter/r-direction-lag-logical-rollback-0511-fix-bust-063119.json` records `ok=true`, `settle.status="settled"`, `pendingPlanAtEnd=null`, final player `{338,270}`, `predictedPlayer=null`, `jumps=[]`, `logicalRollbackWarnings=[]`, `directionLagWarnings=[]`, `consoleErrors=[]`, and `nonFaviconNetwork404s=[]`. Regression evidence: `r-route-spam-obstacle-regression-063209.json` and `r-blocked-target-regression-063209.json` both record `ok=true`, no visual/logical jumps, no route-spam warnings, and explicit `targetBlocked` non-failure status. Verification passed Web `pnpm --dir apps/web exec tsc --noEmit`, `node --check apps/web/scripts/capture-web-movement-jitter.mjs`, and live local Gateway/Web captures at `127.0.0.1:7210` / `127.0.0.1:13010`.
- 2026-05-10 NPC marker/click quest follow-up: the NPC click harness now runs both out-of-range and adjacent MirGuide scenarios and records marker/body/nameplate geometry. Evidence: `docs/generated/player-qa/npc-click/r-npc-click-marker-quest-0511a-summary.json` records `ok=true`; out-of-range `dialogTitle=MirGuide_Peter`, `moveCount=2`, `interactCount=1`; adjacent `dialogTitle=MirGuide_Peter`, `moveCount=0`, `interactCount=1`; marker rect `456,225 28x29`, NPC body rect `440,265 60x80`, `horizontalDeltaPx=0`, `iconBottomToNpcTopPx=-11`, `consoleErrors=[]`, and `nonFaviconNetwork404s=[]`. Verification passed Web `pnpm --dir apps/web exec tsc --noEmit`, `node --check apps/web/scripts/capture-web-npc-click.mjs`, `node --check apps/web/scripts/capture-web-movement-jitter.mjs`, focused `guild_` 15/15, Hero AI 25/25, full locked Simulation 855/855 plus Hero AI 25/25, Gateway shared registry 15/15, package fmt/check, and targeted diff checks.
- 2026-05-10 blocked-target movement settle pass: Player Web now carries short-lived blocked-step memory across repeated target clicks and delays same-source retries until server confirmation/correction, preventing unreachable targets from endlessly resending stale movement vectors or leaving a stale predicted player. The movement jitter harness adds `blockedTarget`, treats blocked/unreachable target residue as an explicit non-failure only when identified, and now asserts no jumps, no route-spam warnings, no console errors, and no non-favicon 404s. Evidence: `docs/generated/player-qa/movement-jitter/r-blocked-target-nonfailure-0511-fixed6.json` records `ok=true`, `settle.status="settled"`, `movementPlan=null`, `predictedPlayer=null`, `directionStepPending=null`, `directionStepPendingQueue=[]`, `jumps=[]`, `routeSpamWarnings=[]`, `consoleErrors=[]`, and `nonFaviconNetwork404s=[]`. Verification passed Web `pnpm --dir apps/web exec tsc --noEmit`, `node --check apps/web/scripts/capture-web-movement-jitter.mjs`, live local Gateway/Web blocked-target capture, focused `guild_` 14/14, Hero AI 23/23, full locked Simulation 854/854 plus Hero AI 23/23, Gateway shared registry 15/15, package fmt/check, and targeted diff checks.
- 2026-05-10 route-spam settle follow-up: route-spam obstacle captures now wait for a final settle phase and record `pendingPlanAtEnd`, distinguishing real unresolved blocked targets from harness window cutoffs. Player Web now rechecks early server corrections after the reroute delay instead of waiting for the long correction grace, and movement packet handling updates the local world ref synchronously so the animation/input loop does not send one extra stale action from an older server tile. Evidence: `docs/generated/player-qa/movement-jitter/r-route-spam-obstacle-settle-followup5.json` records `ok=true`, `settle.status="settled"`, `waitedMs=7`, player `{334,273} -> {334,271}`, `pendingPlanAtEnd=null`, `movementPlan=null`, `predictedPlayer=null`, `directionStepPendingQueue=[]`, `jumps=[]`, `routeSpamWarnings=[]`, `consoleErrors=[]`, and `nonFaviconNetwork404s=[]`. Verification passed Web `pnpm --dir apps/web exec tsc --noEmit`, `node --check apps/web/scripts/capture-web-movement-jitter.mjs`, live local Gateway/Web capture, package fmt/check, Gateway shared registry 15/15, and full locked Simulation 852/852 plus Hero AI 20/20.
- 2026-05-10 route-spam/obstacle input pass: Player Web now stores short-lived blocked route steps after server correction, uses those hints plus visible entity occupancy to choose a walk fallback or nearby reroute direction instead of repeatedly resending the same stale target vector, and reanchors target plans from confirmed server positions after a bounded delay. Self dash/push/attack-move packets and object dash/push packets now flow through the same movement reconciliation path as walk/run/backstep. The movement harness adds `routeSpamObstacle`, runtime exception details, sent-packet probes, and `routeSpamWarnings`. Evidence: `docs/generated/player-qa/movement-jitter/r-route-spam-obstacle-final4.json` records `sampleCount=119`, player `{334,273} -> {334,271}`, `movementPlan=null`, `predictedPlayer=null`, `jumps=[]`, `routeSpamWarnings=[]`, `consoleErrors=[]`, and `nonFaviconNetwork404s=[]`; coordinator reruns `r-route-spam-obstacle-coordinator*.json` also recorded `jumps=[]` and `routeSpamWarnings=[]` with no browser errors. Verification passed Web `pnpm --dir apps/web exec tsc --noEmit`, `node --check apps/web/scripts/capture-web-movement-jitter.mjs`, live local Gateway/Web captures, Simulation/Gateway fmt, locked four-package check, and full locked Simulation 850/850 plus Hero AI 17/17.
- 2026-05-10 client input Crystal-feel pass: Player Web now keeps the local predicted player anchor through the Crystal 600ms direction-step visual window when the server confirms the same tile, instead of clearing prediction immediately and snapping the draw source back to the server tile mid-OffSetMove. The movement jitter harness also makes fixed-map center-sprite jump detection direction-aware, so diagonal `UpRight` Crystal map displacement is not mistaken for rollback while true opposite-sign jumps still fail. Evidence: `docs/generated/player-qa/movement-jitter/r-client-input-crystal-feel-final-diag.json` records a held right-run switched into eight repeated right-click target updates toward `338,266`, final player `338,266`, `movementPlan=null`, `predictedPlayer=null`, `directionStepPendingQueue=[]`, `jumps=[]`, `consoleErrors=[]`, and `nonFaviconNetwork404s=[]`; early samples show the first confirmed `332,268 UpRight` step retaining `predictedPlayer={x:332,y:268,direction:"UpRight"}` after the server `UserLocation` arrives, preserving Crystal `CurrentLocation + OffSetMove` continuity. Verification passed `pnpm --dir apps/web exec tsc --noEmit`, `node --check apps/web/scripts/capture-web-movement-jitter.mjs`, and the live local Gateway/Web capture at `127.0.0.1:13010` / `127.0.0.1:7210`.
- 2026-05-10 client action-feel follow-up: target-click movement plans now inherit the current local action source from an in-flight held-direction queue or predicted position instead of snapping their planning source back to the last server tile, and target plans now apply the same bounded local-lead gate before sending the next run/walk packet. The movement jitter harness exposes `directionStepPendingQueue` and adds `holdThenSpamClickTarget` to stress the hold-to-repeated-click transition. Evidence: `docs/generated/player-qa/movement-jitter/r-client-action-feel-hold-spam-arrive-222639.json` records random account `QA222639`, held-run plus eight repeated right-click target updates, final player `338,270`, `movementPlan=null`, `predictedPlayer=null`, `directionStepPendingQueue=[]`, `jumps=[]`, `consoleErrors=[]`, and `nonFaviconNetwork404s=[]`. Verification passed Web `npx tsc --noEmit`, `node --check apps/web/scripts/capture-web-movement-jitter.mjs`, targeted `git diff --check`, and the live local Gateway/Web capture at `127.0.0.1:13010` / `127.0.0.1:7210`.
- 2026-05-10 NPC click/quest-marker follow-up: Player Web now treats an out-of-range NPC click as a Crystal-style approach-and-call target instead of losing the interaction, then calls the NPC once adjacent; the same-NPC repeat guard was shortened so a failed/no-op click does not swallow the next valid interaction. Quest markers are centered over the NPC body anchor instead of being shifted left by a full icon width. Evidence: `docs/generated/player-qa/npc-click/r-npc-click-after-mirguide-113448.json` records `dialogTitle=MirGuide_Peter`, `interactCount=1`, `moveCount=0`, `consoleErrors=[]`, and `nonFaviconNetwork404s=[]`. Verification passed Web `pnpm --dir apps/web exec tsc --noEmit` plus the live local Gateway/Web NPC click capture.
- 2026-05-10 Crystal input queue refresh: Player Web now treats target-click movement plans and held-direction movement as separate Crystal-style action queues, clearing stale direction confirmations when a new target plan starts, reconciling direction-step confirmations from both live packets and world snapshots, pre-queueing the next input 100ms before the 600ms action window completes, and allowing a bounded four-run-tile local lead while blocking any next command that would exceed that lead. The movement QA harness now includes `spamClickTarget` to stress repeated right-click destination updates. After a clean 13010 Next restart, `docs/generated/player-qa/movement-jitter/r-input-queue-fresh-092543.json` records held right-run sending four `run Right` commands in 1.8s with predicted movement `330,270 -> 338,270` and `jumps=[]`; `docs/generated/player-qa/movement-jitter/r-click-target-fresh-092652.json` records right-click target arrival `330,270 -> 338,270`, `movementPlan=null`, and `jumps=[]`; `docs/generated/player-qa/movement-jitter/r-spam-click-target-before-094503.json` records ten repeated right-clicks on the same target reaching `338,270` with `jumps=[]`. Fresh NPC click evidence at `docs/generated/player-qa/npc-click/r-npc-click-fresh-093512.json` records `dialogTitle=MirGuide_Peter`, `interactCount=1`, `moveCount=0`, `consoleErrors=[]`, and `nonFaviconNetwork404s=[]`. Verification passed Web `pnpm --dir apps/web exec tsc --noEmit`, `node --check apps/web/scripts/capture-web-movement-jitter.mjs`, and live local Gateway/Web movement/NPC captures.
- 2026-05-10 Crystal dash-packet bridge pass: Gateway now serializes `UserDashAttack`, `ObjectDashAttack`, and `UserAttackMove`, and Player Web consumes Crystal self/object dash, dash-fail, dash-attack, attack-move, and push packets through the same server-reconciliation path as walk/run/backstep. Dash and dash-attack packets now drive running-style movement animation instead of being ignored or parsed as `0,0` nested-location payloads. Verification passed Web `pnpm --dir apps/web exec tsc --noEmit` plus focused Simulation `ShoulderDash`, `FlashDash`, and `SlashingBurst` regressions.
- 2026-05-09 component-boundary cleanup pass: Player Web's original-client shell was split into focused component modules without changing gameplay state or movement timing. `apps/web/app/components/original-client-overlays.tsx` now owns login/select/HUD/SpriteButton overlays, `original-client-panels.tsx` owns chat filter/feed, belt, and durability panels, and `original-client-dialogs.tsx` owns Mail, Report, and NPC dialog surfaces. The main shell dropped from 8,795 lines to 7,587 lines while keeping the movement/scene renderer and command queue in one place for the Crystal timing work. Verification passed Web `tsc --noEmit`, direct `next build`, targeted `git diff --check`, `node --check apps/web/scripts/capture-web-npc-click.mjs`, HTTP 200 for `http://127.0.0.1:13010/?gatewayWs=ws%3A%2F%2F127.0.0.1%3A7210%2Fws`, Gateway `/health`, and live browser captures: `docs/generated/player-qa/movement-jitter/r-component-split-input.json` records held right-run `330,270 -> 336,270` with `jumps=[]`; `docs/generated/player-qa/movement-jitter/r-component-split-route.json` records the four-step click route with `jumps=[]`; `docs/generated/player-qa/npc-click/r-component-split-mirguide-click.json` records `dialogTitle=MirGuide_Peter`, `interactCount=1`, `moveCount=0`, `consoleErrors=[]`, and `nonFaviconNetwork404s=[]`.
- 2026-05-09 Crystal input/NPC anchor follow-up pass: Player Web now treats NPC activation like Crystal's `CallNPC` path: clicking an NPC sends one immediate `interact` command with same-NPC throttling while an NPC dialog is open / within the short repeat guard, and no longer starts a client-side auto-approach `moveTo` plan. The client-side movement lead is capped to one pending run/walk action (two tiles) instead of retaining a two-action queue, reducing continuous-click rollback while keeping Crystal's 600ms action cadence. NPC quest-marker CSS now uses a top-left Crystal `DrawLocation + BodyLibrary.GetOffSet + (width/2 - 28, -40)` style anchor instead of centering above the nameplate. Added `apps/web/scripts/capture-web-npc-click.mjs` to verify real DOM NPC clicks. Evidence after a clean Next restart: `docs/generated/player-qa/npc-click/r-crystal-mirguide-click-after.json` records `dialogTitle=MirGuide_Peter`, `interactCount=1`, `moveCount=0`, `consoleErrors=[]`, and `nonFaviconNetwork404s=[]`; `docs/generated/player-qa/movement-jitter/r-crystal-input-after-restart.json` records held right-run `330,270 -> 336,270` with `jumps=[]`; `docs/generated/player-qa/movement-jitter/r-crystal-route-click-after-restart.json` records run/right, run/right, walk/down, walk/left route arrival with `jumps=[]`. Verification passed Web `tsc --noEmit`, `node --check apps/web/scripts/capture-web-npc-click.mjs`, focused Simulation `crystal_manifest_gtmerchant_interact_opens_dialog_when_adjacent`, and live local Gateway/Web browser captures.
- 2026-05-09 movement queued-local confirmation pass: Web held/continuous movement now mirrors Crystal's local `QueuedAction` consumption more closely by allowing the next walk/run action to start on the Crystal 600ms cadence from the last local action position while keeping a bounded two-action / four-tile lead over server confirmation. Server `UserLocation` packets now drain the queued confirmations in order and only clear prediction on timeout/correction, removing the visible pause/rollback that happened when the client waited for the previous confirmation before visually starting the next step. Evidence: `docs/generated/player-qa/movement-jitter/r-live-hold-right-after-queued-local.json` records held right-run moving from `330,270` to `340,270` with `jumps=[]`, `consoleErrors=[]`, and `nonFaviconNetwork404s=[]`; `docs/generated/player-qa/movement-jitter/r-live-click-target-8-after-queued-local.json` records a right-click target path reaching `338,270` with `jumps=[]`; `docs/generated/player-qa/movement-jitter/r-live-click-route-after-queued-local.json` records a run/right, run/right, walk/down, walk/left click route with each segment arriving and `jumps=[]`. Verification passed Web `npx tsc --noEmit`, `git diff --check`, and live local Gateway/Web movement captures.
- 2026-05-08 Stage 5 dirty-save full-smoke hardening pass: Player Web smoke now defaults to a fresh throwaway account, keeping `demo/Scout` clean for human acceptance, while explicit `MIR2_STAGE5_ACCOUNT_MODE=demo` still covers accumulated demo-save state. The smoke validates the real command path for inventory split/use/drop, ground item/gold pickup, storage store/take-back, belt use, NPC services, compact layouts, and late-system System Menu panels. The harness no longer assumes fixed Red/Blue Potion slots or pristine storage: it seeds missing items, uses exact item `uniqueId` / drop `objectId` assertions, checks all inventory containers plus belt where Crystal auto-stack rules can move consumables, and falls back to direct object pickup when a ground marker is not clickable. Evidence updated at `docs/stage5-screenshots/stage5-ui-smoke-manifest.json`: 114 screenshots, `criticalConsoleErrorCount=0`, `compactPanelCount=9`, `compactTextNodeCount=22`, `compactMatrixCount=3`, `systemMenuSocial=44`, `storageTakeBackFlow=4`, `inventorySplitFlow=3`, `groundPickupFlow=3`, and `groundGoldPickupFlow=3`. Verification passed Web `node --check scripts/smoke-stage5-ui.mjs`, Web `npx tsc --noEmit`, and the live Gateway/Web smoke at `http://127.0.0.1:13010/?gatewayWs=ws%3A%2F%2F127.0.0.1%3A7210%2Fws`.
- 2026-05-08 late-dialog command/readiness pass: Player Web System Menu now exposes command-backed Hero and Item Rental panels, and Creature/Mount/Fishing buttons now send real Gateway browser commands instead of being visual-only. The fast Stage 5 smoke verifies recent command history for `updateIntelligentCreature`, equipment `useItem` on mount slot, `fishingCast`, `fishingChangeAutocast`, `newHero`, and `itemRentalRequest`; the System Menu social coverage now includes Hero and ItemRental rows, raising fast-smoke social states from 36 to 44. Simulation also exposes `stage5Systems.itemRental` in world snapshots so the rental panel can read partner, fee, period, deposited item, lock state, and rented records. Evidence: live local Gateway/Web smoke passed with 22 screenshots, `systemMenuFeature=10`, `systemMenuSocial=44`, and `systemMenuQaTransfer=3`. Verification passed Web `node --check scripts/smoke-stage5-ui.mjs`, Web `npx tsc --noEmit`, focused Simulation `item_rental_` 3/3, locked Simulation/Gateway check, and Gateway browser-command mapping 7/7. Human Crystal dialog/pixel acceptance remains open.
- 2026-05-08 map torch/fire light offset pass: Web map object rendering now preserves Crystal Lib frame offset metadata for exported/dynamically loaded map frames and applies the Crystal offset mode to the Bichon `Objects/2723-2732` torch/fire blend frames in the packaged starter-map fallback. This moves the visible torch/fire light layer from the right/down neighboring cell back onto the red torch head, and routes those blend frames through generated additive-clean PNGs so the original dark edge pixels no longer render as a black ring. Evidence: `docs/generated/player-qa/light-offset/r-light-offset-torch-clean-state.json` records player `0:336,278`, torch body `Objects/2733` cell `335,274` rect `left=422 top=118`, and torch fire/light `Objects/2730` cell `336,275` rect `left=420 top=88` with `renderPath=/generated/original-map-blend/WemadeMir2/Objects/2730.png`, `mixBlendMode=screen`, `consoleErrorCount=0`, and `nonFaviconNetwork404s=[]`; screenshot evidence is `docs/generated/player-qa/light-offset/r-light-offset-torch-clean.png`. Verification passed Web `npx tsc --noEmit`, blend-asset generation/script syntax checks, and focused live capture against local Gateway/Web.
- 2026-05-08 movement feel cleanup pass: Web scene motion now uses Crystal-style six-frame walk/run displacement for the camera/entity offset, quantizes movement offsets to the same even-pixel cadence as Crystal `OffSetMove`, and caps held-direction local prediction to one unconfirmed walk/run action instead of letting repeated right-hold samples drift many tiles ahead of the server. The movement capture harness can now create isolated QA accounts/characters and records command/gateway movement tails for diagnosis. Evidence: `docs/generated/player-qa/movement-jitter/r-movement-direction-pending-cleared-after.json` records held right-run with `jumps=[]`, derived maximum prediction lead of 2 tiles, integer/even `centerSprite` deltas, `consoleErrors=[]`, and `nonFaviconNetwork404s=[]`; `docs/generated/player-qa/movement-jitter/r-movement-click-direct-crystal-frame-after.json` records direct run/walk route replay with `jumps=[]`, derived maximum prediction lead of 0 tiles, and no browser errors. Verification passed Web `npx tsc --noEmit`, movement capture script syntax, and live local Gateway/Web captures.
- 2026-05-07 Stage 5 frontend 2/4/5/6 closure pass: Player Web now consumes live Crystal magic/combat visual packets (`Magic`, `MagicCast`, `MagicDelay`, `MagicLeveled`, `ObjectMagic`, `ObjectProjectile`, `MapEffect`, `AddBuff`, `RemoveBuff`, `PauseBuff`) instead of relying only on snapshots, applies Crystal-like action timing for melee/range/struck/death windows, and keeps spell/buff cooldown state visible through the HUD skill surface. Late-system UI coverage now includes a real `trade` chat filter, dynamic System Menu social panels for ranking/friend/group/guild/trade/market/marriage/mentor/relationship, and command dispatch for supported Stage 5 social/trade/market actions. NPC/quest smoke now opens InnKeeper_Brittney through the Crystal dialog path, strips raw script markup from visible dialog text, exposes dialog/quest state to QA, routes selected-NPC primary actions through approach handling, and verifies Quest Diary row title/stage/progress/reward content. Responsive coverage now runs a compact matrix across 900x640, 768x640, and 820x540, adds overflow-safe CSS for mail/storage/system/social/quest text, and anchors the screenshot output directory to the repo path. Evidence: full live Stage 5 UI smoke against an isolated Gateway captured 113 screenshots with `criticalConsoleErrorCount=0`, `compactMatrixCount=3`, `systemMenuSocial=36`, `systemMenuFeature=6`, `storagePassword=9`, `npcDialogFlow=11`, and `combatFlow=2`; verification passed Web `npx tsc --noEmit`, smoke script syntax, and the live isolated-Gateway smoke.
- 2026-05-07 movement/animation Crystal-timing pass: Player Web now records walk/run action windows from `UserLocation` / `ObjectWalk` / `ObjectRun` packets and drives entity sprite frames from the action start time instead of the global scene frame. Player, monster, and NPC standing frames now use the Crystal frame intervals (`Player`/monster 4 frames at 500ms, NPC 4 frames at 450ms), movement actions use the 6-frame/600ms Crystal timing window, and the browser-only attack bounce transform was removed so attack motion comes from the source sprite frames. Evidence: `docs/generated/player-qa/movement-jitter/r-movement-animation-crystal-timing.json` against local Gateway/Web with `demo/demo` records held right-run from `330,270` to `338,270`, `jumps=[]`, `consoleErrors=[]`, and `nonFaviconNetwork404s=[]`; `r-movement-animation-crystal-timing-live-tick.json` keeps automatic Gateway ticks enabled and records concurrent `ObjectWalk` monster movement packets with `jumps=[]`, `consoleErrors=[]`, and `nonFaviconNetwork404s=[]`. Verification passed Web `npx tsc --noEmit` and movement capture script syntax.
- 2026-05-07 Stage 5 full-smoke stabilization pass: the deterministic Player Web smoke now disables automatic keep-alive/tick flooding with `autoTick=0`, captures WebSocket frame diagnostics for timeout triage, opens a real Crystal NPC dialog through `qa.openNpcDialog`, and drives combat through a Crystal-backed `BugBat` event spawn placed on a nearby spawnable map tile. Full `smoke:stage5-ui` against an isolated local Gateway captured 102 screenshots with `criticalConsoleErrorCount=0`; manifest evidence records `storagePassword=9`, `storageStoreFlow=4`, `storageTakeBackFlow=4`, `characterRepairFlow=8`, `gameShopFlow=4`, `beltUseFlow=7`, `beltMouseUseFlow=3`, `npcDialogFlow=11`, and `combatFlow=2`. Verification passed Web `npx tsc --noEmit`, smoke script syntax, Rust fmt/check for Simulation/Gateway, full locked `mir2-gateway` 107/107 plus packet-trace bin 17/17, full locked `mir2-simulation` 731/731, shared in-process registry 11/11, and the live isolated-Gateway Stage 5 UI smoke.
- 2026-05-07 service-backed storage/repair, GameShop, and belt full-hotkey pass: Player Web can now target an isolated Gateway through `?gatewayWs=` / `NEXT_PUBLIC_MIR2_GATEWAY_WS_URL`, allowing deterministic Stage 5 UI smoke against a fresh account store without mutating the developer's default `7110` store. Full `smoke:stage5-ui` captured 101 screenshots with `criticalConsoleErrorCount=0`; the manifest records `storagePassword=9`, `storageStoreFlow=4`, `storageTakeBackFlow=4`, `beltUseFlow=7`, `characterRepairFlow=8`, and `gameShopFlow=4`. The smoke opens InnKeeper_Brittney's Crystal `@Storage` service, verifies service-backed storage password set/unlock/change/remove including persisted last-set timestamp exposure, stores Dagger into warehouse slot 4, takes Red Potion back from storage into inventory, damages equipped Dagger through deterministic QA setup, repairs it through Blacksmith_Smith `@Repair` and Blacksmith_Bill `@SRepair` with durability/gold assertions, seeds gold through real Mail claim, buys `AccuracyPotion` from GameShop with Gold and verifies carry-slot delivery plus purchase feedback, presses belt hotkeys `1..6`, and confirms the Crystal Mail row no longer emits nested-button hydration errors. Evidence: `docs/stage5-screenshots/stage5-ui-smoke-manifest.json` plus new screenshots including `stage5-storage-service-npc.png`, `stage5-storage-password-set.png`, `stage5-storage-password-unlocked.png`, `stage5-storage-password-changed.png`, `stage5-storage-password-removed.png`, `stage5-storage-service-return.png`, `stage5-repair-service-npc.png`, `stage5-special-repair-service-npc.png`, `stage5-gameshop-gold-open.png`, and `stage5-gameshop-gold-buy.png`.
- 2026-05-07 P1/P2 social System Menu pass: the Crystal-style narrow Menu social branch now opens player-facing group, guild, mentor, relationship, ranking, keyboard/help, and report/admin surfaces without visible Web/QA placeholder wording. The backend packet-runtime round that backs these surfaces also added typed Group utility, Quest, Refine, Market, OpenDoor, and request-info behavior. Verification passed Web `npx tsc --noEmit`, script syntax check, and a fast live Stage 5 UI smoke against local Gateway/Web with 17 screenshots, including `systemMenuSocial=24`, `systemMenuFeature=6`, and `systemMenuQaTransfer=3` manifest counts. Human Crystal visual/feel acceptance for exact dialog bitmaps remains open.
- 2026-05-01 R327 Gameshop purchase and map-click arrival pass: Gameshop Buy is now wired from the Crystal product cell to `gameShop.buyCredit` / `gameShop.buyGold`, with Web state exposing account credit and backend handling purchases from the generated Crystal game-shop manifest. Browser evidence for `QA0429A / QA0429Hero` verifies the first product (`AccuracyPotion`) sends `gameShop.buyCredit` with args `20,1`; because the QA account has `credit=0`, the expected insufficient-currency message is shown. Positive credit delivery is covered by the focused simulation test, which deducts credit and delivers the manifest-backed item by Stage 5 mail. Map click-to-arrive now holds only one pending target step ahead of server confirmation, reconciles self `ObjectRun` / `ObjectWalk` packets immediately, and removes the 180ms movement-time tick flood that was delaying queued `moveTo` packets behind monster updates. Evidence: `docs/generated/player-qa/r327-gameshop-buy-click-final-clean-state.json` records `network404Count=0`, `consoleErrorCount=0`, `gameShop.visible=true`, and the `gameShop.buyCredit` command; `docs/generated/player-qa/movement-jitter/r327-map-click-target-arrival-fixed3.json` records right-click target `338,270`, final player `338,270`, `movementPlan=null`, and `jumps=[]`. Verification passed: web `tsc --noEmit`, capture-script syntax checks, focused game-shop simulation test, `mir2-gateway` check, and targeted CDP captures. `NPC/25` was exported from the Crystal client to remove the prior resource 404 in this scene.
- 2026-05-01 R326 held-mouse queued-action input fix: Web now separates Crystal-style held input sampling from action execution. The scene still samples the held pointer every 100ms, but `page.tsx` keeps the latest pending direction request instead of dropping samples during the 600ms walk/run action window. When the current movement action can be consumed, Web uses the latest queued direction to send one Crystal-like `Walk` / `Run` packet, matching the original client's `User.QueuedAction` overwrite/consume model without sending movement packets at a 100ms speed. Evidence: `docs/generated/player-qa/movement-jitter/r326-web-hold-run-queued-direction.json` records held right-run from `332,270` to `344,270`, `movementPlan=null`, fixed map-sprite continuity, and `jumps=[]`; `r326-web-hold-run-ws-send-probe.json` records WebSocket `sentMoveTail` entries for repeated `{"type":"run","direction":"Right"}` with `jumps=[]`. Gateway movement logs with `MIR2_GATEWAY_MOVE_LOG=1` show continuous `Run direction=Right -> UserLocation=(332..344,270)`. Verification passed: web `tsc --noEmit`, capture-script syntax check, and targeted CDP movement captures. The prior `original-ui/NPC/25/meta.json` 404 seen in this area was removed by the R327 asset export.
- 2026-05-01 R325 held-run visual jitter fix: fixed the remaining user-reported movement stutter/backtrack during held right-button running. The root cause was a one-frame mismatch between the newly predicted player tile and the previous motion snapshot: map sprites were rebuilt around the predicted tile while camera interpolation still used the old snapshot, so fixed floor sprites could jump right by roughly 2 tiles at server-confirmation boundaries. Web now keeps predicted movement in the render pipeline, uses the predicted player as the viewport/map/entity basis, and refreshes entity motion snapshots synchronously before rendering so map/camera math cannot use mixed old/new movement targets. The movement capture now records fixed map-sprite keys to catch background backtracking. Evidence: `docs/generated/player-qa/movement-jitter/r325-web-hold-run-final-4s.json` records held right-run from `332,270` to `344,270`, `movementPlan=null`, fixed sprite `sprite-3:330:270:0` moving monotonically, and `jumps=[]`; `r325-web-hold-run-sync-motion-snapshot.json` also records `jumps=[]` after the synchronous snapshot fix. Gateway move logging remains gated by `MIR2_GATEWAY_MOVE_LOG=1` and shows continuous `Run direction=Right -> UserLocation=(332..344,270)`. Verification passed: web `tsc --noEmit`, `node --check apps\web\scripts\capture-web-movement-jitter.mjs`, `cargo +1.89.0 check --locked -p mir2-gateway`, and targeted CDP movement captures. The known unrelated `original-ui/NPC/25/meta.json` 404 remains in movement captures.
- 2026-05-01 R322 movement correction loop fix: fixed the user-reported severe movement back-and-forth loop by removing predicted coordinates as the logical source for the next movement step. Prediction is now visual-only, and the movement plan records the pending server step; if `UserLocation` / snapshot correction does not land on that pending tile after the step window, Web clears the plan and prediction instead of repeatedly chasing the old target. This prevents client prediction and server correction from pulling the player in opposite directions around blocked/partially blocked Bichon tiles. Evidence: `docs/generated/player-qa/movement-jitter/r322-web-movement-no-predict-source.json`, `r322-web-movement-correction-stop.json`, and `r322-web-movement-open-area.json`; all record `jumps=[]`. Verification passed: web `tsc --noEmit` and targeted movement captures. Remaining movement feel work is Crystal-like continuous held-button `QueuedAction` behavior, but the oscillating loop is fixed.
- 2026-05-01 R323 held-mouse `QueuedAction` movement pass: Web now mirrors the original client's held mouse loop more closely. The scene input layer samples the held pointer every 100ms, stores coordinates in the Crystal 1024x768 stage coordinate system, and held walk/run dispatches Crystal-like `Walk` / `Run` direction packets instead of repeatedly feeding absolute `moveTo` path targets. The frontend keeps a local action coordinate for the next queued step, while server `UserLocation` / snapshots still reconcile or clear prediction on correction. Evidence: `docs/generated/player-qa/movement-jitter/r323-web-hold-run-direct-direction.json` records a 2.2s right-hold run from `330,270` to final `340,270` with `jumps=[]`; `r323-web-hold-walk-direct-direction.json` records left-hold walk to `335,270` with `jumps=[]`; `r323-web-packet-run-right.json` verifies repeated raw `Run Right` packets also reach `340,270`. Known non-movement warning in the captures: missing `original-ui/NPC/25/meta.json` still produces a 404. Remaining movement acceptance is human feel comparison against original Crystal, plus deeper collision/blocked-tile edge parity.
- 2026-05-01 R321 original/Web movement-control baseline: added `apps/web/scripts/capture-original-movement.ps1` so automation can bring the original Crystal `Legend of Mir 2` window to the foreground, send client-coordinate left/right mouse actions, and archive timed client-area screenshots next to Web movement evidence. R321 evidence under `docs/generated/player-qa/movement-jitter/` confirms original control captured 15 frames at 1024x768 for `QA0429Hero`, Web direct gateway `moveTo` moved cleanly through run/walk steps with `jumps=[]`, and Web real tile click/right-click had `jumps=[]`. A real hit-test bug was fixed by moving `.tile-hit` with the same `playerCameraMotionOffset` as the rendered map/entities; after the fix `r321-web-movement-click-hitoffset.json` shows the first right-click immediately advances from `330,270` to `332,270`. Crystal source comparison confirms the original client drives movement from `GameScene.Process` 100ms `CanMove` ticks plus `PlayerObject.SetAction()` consuming `QueuedAction`, immediately updating `CurrentLocation` and using server packets as correction; Web still uses a simplified target-plan loop, so the remaining movement-feel issue is continuous input/action-queue parity rather than DOM jitter or backend traversal failure.
- 2026-04-30 R319 label/BigMap/Mail/cursor parity pass: tightened the latest user-reported visual gaps against Crystal source. Entity nameplates now keep Crystal-style object-centered labels: NPC/monster underscore names split into stacked lines (`Teleport` / `Gilbert`) instead of web-normalized prose, and selected target HP/action hints no longer enlarge the name label. BigMap NPC rows now use the whole-map Crystal NPC manifest, render `MapLinkIcon` sprites, and format names like `(Teleport)Gilbert`; Mail empty state no longer shows Web `No mail`; and the client stage uses Crystal cursor files (`Cursor_Default.CUR`, `Cursor_Npc.cur`, `Cursor_Normal_Atk.CUR`, `Cursor_TextPrompt.CUR`). Evidence: `docs/generated/player-qa/r319-label-bigmap-mail-cursor/r319-label-bigmap-mail-cursor-final.png` and `docs/generated/player-qa/r319-label-bigmap-mail-cursor/r319-label-bigmap-mail-cursor-final-state.json`; state records `mailPanel.emptyVisible=false`, `bigMap.npcRowCount=18`, first BigMap rows `(Teleport)Gilbert`, `(BorderVillage)Board`, `(Assistant)Jane` with `/original-ui/MapLinkIcon/*.png`, Crystal `.CUR` cursor CSS for stage/NPC/monster hits, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Verification passed: UI asset export, web `tsc --noEmit`, capture script syntax check, and focused CDP capture with `--openMail true --openBigMap true`.
- 2026-04-30 R318 BigMap/MailList parity pass: fixed the user-reported BigMap and Mail UI mismatch by replacing Web-style surfaces with Crystal source-aligned dialogs. The minimap BigMap button now toggles a real `BigMapDialog` using exported `Title/820`, `Title/821-829`, `Prguse2/1340-1342`, `Prguse2/1350`, and world-map `Prguse2/1360/1365/1366` assets; Web stores `MapInformation.bigMapIndex` and draws the current Crystal big-map raster with source dialog controls, title, coordinate label, NPC rows, and radar dots. The Mail button now opens the Crystal `MailListDialog` frame `Title/670` at the original 1024x768 position with `Title/7`, close/help/page/action buttons, 10-row layout, row icons/flags, and no visible Web overlay header. Evidence: `docs/generated/player-qa/r318-mail-bigmap/r318-mail-bigmap-final.png` and `docs/generated/player-qa/r318-mail-bigmap/r318-mail-bigmap-final-state.json`; state records `mailPanel.bounds=562,5,312,444`, `mailPanel.hasFrame=true`, `mailPanel.visibleOverlayHead=false`, `mailPanel.oldOverlayRowCount=0`, `bigMap.bounds=132,134,760,500`, `bigMap.viewport=146,186,568,380`, `bigMap.hasFrame=true`, `bigMap.hasRaster=true`, `bigMap.title="BichonProvince"`, `bigMap.coordinate="[ 287, 618 ]"`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Verification passed: UI asset export, web `tsc --noEmit`, capture/smoke script syntax checks, focused CDP capture with `--openMail true --openBigMap true`, and `git diff --check`.
- 2026-04-30 R317 Gameshop product-grid parity pass: replaced the remaining Web placeholder Gameshop interior with Crystal-backed product data and original cell/button assets. Web now renders the 105-item generated Crystal Gameshop manifest through the original `Title/750` cell frame, `Title/778-783` buy/preview buttons, `Prguse2/240-245` quantity controls, real category filters, class tabs, search, page labels, payment checkboxes, stock/count/price labels, and item icons exported from `Items.Lib`. Evidence: `docs/generated/player-qa/r317-gameshop-products/r317-gameshop-products.png` and `docs/generated/player-qa/r317-gameshop-products/r317-gameshop-products-state.json`; state records `gameShop.bounds=164,70,696,476`, `cellCount=8`, `firstCellName="AccuracyPotion"`, `pageLabel="1 / 14"`, `categoryCount=10`, `loadedIconCount=8`, `buyButtonCount=8`, `previewButtonCount=1`, `oldPlaceholderCellCount=0`, `inventoryVisible=false`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Verification passed: web `tsc --noEmit`, capture-script syntax check, R317 CDP capture with `--openGameShop true`, UI asset export, and `git diff --check`.
- 2026-04-30 R316 Gameshop/Menu parity pass: fixed the user-reported HUD Gameshop/Menu mismatch. Crystal source confirms the HUD Gameshop button toggles `GameShopDialog` and the Menu button toggles the narrow `MenuDialog`; Web had Gameshop incorrectly wired to `onOpenInventoryTab("quest")` and rendered Menu as a large debug/transfer panel. Web now opens a Crystal-framed `GameShopDialog` shell from the Gameshop button without opening Inventory, and Menu renders the exported `Title/567` 36x282 vertical icon strip with 13 original sprite buttons at Crystal offsets. The QA transfer form is still available to automation but is moved offscreen so it no longer appears as the normal player menu. Exported missing Crystal UI assets include Gameshop frame/tabs/buttons and Menu frame/icon triples. Evidence: `docs/generated/player-qa/r316-gameshop-menu/r316-gameshop-open.png`, `docs/generated/player-qa/r316-gameshop-menu/r316-menu-open.png`, and `docs/generated/player-qa/r316-gameshop-menu/r316-gameshop-menu-state.json`; state records `shopVisible=true`, `inventoryVisible=false`, `shopBounds=164,70,696,476`, `menuBounds=988,349,36,282`, `iconCount=13`, `oldOverlayHeadVisible=false`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Verification passed: web `tsc --noEmit`, capture-script syntax check, focused CDP click capture, and `git diff --check`.
- 2026-04-30 R315 empty new-character panel-state pass: closed the data-state cause behind the latest character/equipment, inventory, spells, quest, and storage screenshots. Crystal source confirms new character arrays for inventory/equipment/quest inventory/magics are empty and account gold/storage start empty unless real `StartItems` exist. Web runtime now creates real `NewCharacter` saves with empty bag/belt/storage/equipment/quest/skill state and `gold=0`, no longer treats empty save arrays as a signal to refill Web demo seeds, migrates old level-1 exact Web seed saves to empty Crystal state, and preserves `demo/Scout` Stage 5 seed data for automation. Character Spells no longer fills empty magic rows with Web hints/buffs, and the web-only repair/special-repair buttons were removed from the Character page. Evidence: `docs/generated/player-qa/r315-empty-new-character-panels/r315-empty-new-character-panels.png` and `docs/generated/player-qa/r315-empty-new-character-panels/r315-empty-new-character-panels-state.json`; state records `gold=0`, `inventoryItemCount=0`, `beltItemCount=0`, `storageItemCount=0`, `equipmentItemCount=0`, `questCount=0`, `skillCount=0`, `hudHealthOnlyLabel="HP 18/18"`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Verification passed: focused `mir2-simulation start_game_` 16/16, `mir2-gateway` build, web `tsc --noEmit`, R315 CDP capture, `fmt --check`, and capture-script syntax check. Remaining panel visual work is exact Quest Diary/Storage bitmap layout and character paperdoll base sprite details.
- 2026-04-30 R314 HUD/chat/belt/vitals parity pass: closed the user-reported same-scene HUD text/value gap for the aligned Bichon comparison. Web now uses Crystal `MainDialog` HP-only behavior for low-level Warriors with exported `Prguse` frame 6, renders the HP label as `HP current/max`, keeps the Crystal bitmap orb fill from R311, and layers the belt bar from `Prguse` 1932 plus the 0.5-opacity 1933 overlay with 32px slots at Crystal offsets. Chat now follows Crystal's 4 visible rows, 13px row height, Arial 8pt-like sizing, white/blue/red row backgrounds, and Crystal-style hint/server colors. The backend default/legacy-save vitals now come from Crystal `BaseStats` formulas, so `QA0429A / QA0429Hero` at level 1 records `playerHp=18`, `playerMaxHp=18`, `playerMp=14`, and `hudHealthOnlyLabel="HP 18/18"`. Evidence: `docs/generated/player-qa/r314-crystal-vitals-hud/r314-bichon-287-618-vitals-hud.png` and `docs/generated/player-qa/r314-crystal-vitals-hud/r314-bichon-287-618-vitals-hud-state.json`, with exact 1024x768 stage/HUD bounds, 4 chat lines, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Verification passed: focused `mir2-simulation start_game_` 15/15, `cargo +1.89.0 build --locked -p mir2-gateway`, web `tsc --noEmit`, R314 CDP capture, `cargo +1.89.0 fmt --check`, and `git diff --check`.
- 2026-04-30 R312 entity projection/nameplate source alignment: reconciled the R311 same-scene framing change against Crystal source. Web map floor/object sprites keep Crystal `MapControl` map-layer math, while entity sprites/nameplates now use the Crystal `DrawLocation` origin (`OffSetX * 48`, `OffSetY * 32`) and `DisplayRectangle`-relative name/health placement instead of centering entity stacks on tile centers or applying a web-only self-nameplate offset. The vertical viewport offset is restored to Crystal's `Settings.ScreenHeight / 2 / CellHeight - 1` formula, and the focused capture at `BichonProvince` map `0`, `287,618` records `QA0429Hero` nameplate `top=275` with exact `1024x768` stage bounds, HUD `0,616,1024,768`, `questMarkerCount=0`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Evidence: `docs/generated/player-qa/r312-entity-crystal-anchor/r312-bichon-287-618-entity-anchor.png` and `docs/generated/player-qa/r312-entity-crystal-anchor/r312-bichon-287-618-entity-anchor-state.json`. R312 supersedes the R311 playfield-centered camera experiment for projection math; the R311 Crystal bitmap HUD orb fill remains in place.
- 2026-04-30 R311 playfield camera/HUD orb cleanup: the Web Bichon comparison camera now centers on the Crystal playfield height above the 152px HUD instead of the full 768px client stage, moving `QA0429Hero` from the R310 web nameplate `top=389` to `top=325` at `BichonProvince` map `0`, `287,618`, much closer to the original-client same-scene framing. The main HUD HP/MP orb fill now uses the exported Crystal `Prguse` frame 4 left/right orb halves instead of CSS gradients; `Prguse` frames 4 and 6 were added to the UI export manifest. Evidence at `docs/generated/player-qa/r311-playfield-camera/r311-bichon-287-618-hud-orb-state.json` records exact `1024x768` stage bounds, `hud=0,616,1024,768`, `questMarkerCount=0`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`; screenshots are `r311-bichon-287-618-playfield-camera.png` and `r311-bichon-287-618-hud-orb.png`. This reduces the user-reported same-scene visual mismatch; remaining visual acceptance still includes exact dynamic placement, lighting/effects, chat/HUD text feel, and human final acceptance.
- 2026-04-29 R310 original/Web visual-watch bootstrap: added repeatable same-scene Web capture at `apps/web/scripts/capture-crystal-parity.mjs` and a six-hour-capable original/Web sampler at `apps/web/scripts/r310-visual-watch.ps1`. R310 fixes the login-success transition leak so the Mir login animation is cleared before `screen=game`, and scopes NPC quest markers to NPCs whose server snapshot `questIds` match the active quest instead of painting every NPC. Evidence at `docs/generated/player-qa/r310-visual-watch/r310-final-web-scene-state.json` records `QA0429A / QA0429Hero` at `BichonProvince` map `0`, `287,618` with `transitionOverlayVisible=false`, `questMarkerCount=0`, `stage=0,0,1024,768`, `hud=0,616,1024,768`, `miniMap.right=1024`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`; screenshot evidence is `docs/generated/player-qa/r310-visual-watch/r310-final-web-scene.png`. A one-sample original/Web watch test wrote `watch-20260429-042013-original.png`, `watch-20260429-042013-web.png`, and `r310-visual-watch-log.jsonl` with no errors. This is automated comparison evidence only; human final visual acceptance remains open.
- 2026-04-29 R309 Bichon minimap/HUD bounds cleanup: the aligned Bichon desktop minimap frame no longer overflows the 1024x768 Crystal-size stage by 2px. `.mini-map-panel` now sits at `left=896`, `right=1024`, `width=128` in desktop evidence, while compact `820x640` evidence keeps minimap and core HUD bounds inside the viewport. Evidence at `docs/generated/player-qa/r309-minimap-bounds-web-page-state.json` records `desktopOverflows=[]`, `compactOverflows=[]`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]` for `QA0429A / QA0429Hero` at `BichonProvince` map `0`, `287,618`; screenshots are `docs/generated/player-qa/r309-minimap-bounds-web-page.png` and `docs/generated/player-qa/r309-minimap-bounds-compact-web-page.png`. This closes the measured minimap boundary overflow; exact dynamic animal density/placement and human visual acceptance remain open.
- 2026-04-29 R308 Bichon viewport/resource scale cleanup: the web game stage no longer applies the 0.9 browser-only downscale at original comparison sizes, the outer page/frame background is now plain black with no decorative shadow, and compact scaling is reserved for viewports smaller than the 1024x768 Crystal client frame. The same Bichon comparison point also now has exported original sprite meta/assets for previously missing `NPC/00`, `NPC/01`, `NPC/03`, `NPC/11`, `NPC/15`, `Monster/003`, `Monster/004`, and `Monster/005`. Evidence at `docs/generated/player-qa/r308-stage-scale-web-page-state.json` records `QA0429A / QA0429Hero` at `BichonProvince` map `0`, `287,618`; desktop stage bounds are exactly `0,0,1024,768` with transform scale 1, compact bounds are `798.72x599.04` inside `820x640`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Screenshots: `docs/generated/player-qa/r308-stage-scale-web-page.png` and `docs/generated/player-qa/r308-stage-scale-compact-web-page.png`. This closes the browser-only stage-scale/frame decoration gap for the comparison viewport and removes the visible-object sprite 404s; exact dynamic animal density/placement and human visual acceptance remain open.
- 2026-04-29 R307 Bichon guard/archer comparison point evidence: added a focused simulation regression for the aligned Bichon `0:287,618` comparison point so imported fixed respawns must expose `Guard` at `291,620` and `ArcherGuard` at `295,624` through both `ObjectMonster` packets and `worldSnapshot`. Browser evidence at `docs/generated/player-qa/r307-bichon-guard-archer-web-page.png` plus `docs/generated/player-qa/r307-bichon-guard-archer-web-page-state.json` records `hasGuard=true`, `hasArcherGuard=true`, `monsterCount=7`, `npcCount=5`, `hasUnderscoreNameplate=false`, and `questTrackerVisible=false` for `QA0429A / QA0429Hero` at `BichonProvince` map `0`, `287,618`. Verified with focused `mir2-simulation` regression and CDP browser capture with zero console errors. This closes the ordinary Guard/ArcherGuard visibility evidence at the user's second Bichon comparison point; exact dynamic animal density/placement, HUD scale/letterboxing, and human visual acceptance remain open.
- 2026-04-29 R306 Bichon same-scene display cleanup: default game view no longer renders the web-only left quest tracker panel over the Crystal playfield, and NPC/monster nameplates now display Crystal-style space-separated names while keeping raw runtime entity names unchanged for packets/tests. Browser evidence at `docs/generated/player-qa/r306-bichon-display-web-page.png` plus `docs/generated/player-qa/r306-bichon-display-web-page-state.json` records `entityCount=17`, `npcCount=8`, `monsterCount=8`, `npcSpriteElementCount=8`, `monsterSpriteElementCount=8`, `hasUnderscoreNameplate=false`, and `questTrackerVisible=false` for `QA0429A / QA0429Hero` at `BichonProvince` map `0`, `284,607`. Verified with web `tsc --noEmit` and CDP browser capture with zero console errors. Exact object placement/density, HUD scale/letterboxing, and human visual acceptance remain open.
- 2026-04-29 R305 Bichon same-scene visible respawn fix: current-map Crystal visible respawns now enter the ECS world snapshot, not only the bootstrap `ObjectMonster` packet stream. Same-coordinate WS evidence at `docs/generated/player-qa/r305-bichon-visible-world-snapshot.json` records `entityCount=17`, `npcCount=8`, `monsterCount=8`, including `Deer`, `Scarecrow`, `Hen`, and two `Royal_Guard` entities around `QA0429Hero` at `0:284,607`. Browser evidence at `docs/generated/player-qa/r305-bichon-visible-web-page.png` plus `docs/generated/player-qa/r305-bichon-visible-web-page-state.json` records `npcSpriteElementCount=8` and `monsterSpriteElementCount=8`. Verified with focused R305 simulation regression, the existing visible-respawn density regression, `fmt --check`, `mir2-gateway` build, live WS probe, browser state/screenshot capture, gateway health, and web HTTP 200. This closes the first obvious Deer/Royal Guard gap from the Bichon screenshots; broader visual 1:1 remains open for exact density, ordinary guard/archer placement, name normalization, quest tracker/HUD scale, and human screenshot acceptance.
- 2026-04-29 R304 Bichon same-scene NPC population fix: starting a saved web character on a real Crystal map now rebuilds the runtime world from the current map and instantiates Crystal NPC-info manifest entries before the web snapshot is emitted. Same-coordinate WS verification for `QA0429A / Mir2test1 / QA0429Hero` at `BichonProvince` map `0`, `284,607` is archived at `docs/generated/player-qa/r304-bichon-npc-world-snapshot.json`: `entityCount=9`, `npcCount=8`, and `Assistant_Jane` plus `Merchant_Ruben` are present. Browser CDP evidence is archived at `docs/generated/player-qa/r304-bichon-npc-web-page.png` plus `docs/generated/player-qa/r304-bichon-npc-web-page-state.json`; the page state records `npcCount=8`, `npcSpriteElementCount=8`, and visible nameplates for the expected Bichon NPCs. Verified with focused/adjacent `mir2-simulation` tests, `cargo +1.89.0 fmt --check`, `cargo +1.89.0 build --locked -p mir2-gateway`, a live WS probe against gateway `127.0.0.1:7110`, and browser state/screenshot capture against `http://127.0.0.1:3002`. R305 later added visible respawns; visual 1:1 still remains open for exact object density, NPC display-name normalization, quest tracker overlay, HUD scale/letterboxing, and human screenshot acceptance.
- 2026-04-29 login/select audio bootstrap: exported Crystal `Sound/Login2.wav`, `Sound/Select2.wav`, and `Sound/100.wav` into the web public assets. Web now loops login music on the login scene, keeps it through the login-success transition while playing the login effect, then switches to select music. Browser autoplay may defer the first play until a user click/key gesture. Verified with `.\node_modules\.bin\tsc.cmd --noEmit` from `apps\web` plus HTTP 200 checks for all three WAV assets.
- 2026-04-29 login transition fix: web login now holds the first `ChrSel` frame while idle and plays the 19-frame login transition once when leaving the login screen after successful entry. Verified with `.\node_modules\.bin\tsc.cmd --noEmit` from `apps\web`; this does not close Crystal pixel/feel acceptance.
- 2026-04-29 same-scene manual-comparison setup: original Crystal `QA0429A / Mir2test1 / QA0429Hero` is mirrored into the web account store, with the web character aligned to `BichonProvince` map `0` at `287,618`. Frontend CDP verification reached `screen=game`, `mapFileName=0`, `mapTitle=BichonProvince`, `player=287,618` and archived `docs/generated/player-qa/latest-web-align-qa0429a.png` plus `docs/generated/player-qa/latest-web-align-qa0429a-frontend.json`. This opens an apples-to-apples human comparison point; it does not close visual 1:1 because NPC/monster population, quest panel visibility, outer scale/letterboxing, and HUD details still need judgment/fixes.
- 2026-04-29 R303 map-resource audit: `npm.cmd run audit:crystal-map-coverage --prefix apps\web` wrote `docs/generated/map/latest-crystal-map-coverage.json` and archived `docs/generated/map/r303-crystal-map-coverage.json`. This first audit confirmed 463/463 Crystal manifest maps had local source map files, 0 unsupported map types, 0 parse errors, and 463/463 sampled viewports with source frames. Its then-open source-frame and minimap warnings are superseded by the 2026-05-16 all-map resource/gameplay audit above.
- 2026-04-28 R302 original-client comparison: original Crystal `Server.exe` and visible `Client.exe` were launched locally, a retained Crystal QA character was created, and select/game screenshots were archived under `docs/generated/player-qa/r302-original-client/`. Web Stage 5 UI smoke was refreshed from `http://127.0.0.1:3002` with 88 screenshots and 0 critical console errors. R302 confirms original-client visual-reference capture is possible; it does not close the frontend rows because same-scene visual/feel acceptance is still human-blocked or must be explicitly accepted.
- 2026-04-28 R301 automation refresh: final Candidate acceptance pack passed and is summarized in `docs/generated/player-qa/r301-summary.json`. Web `tsc --noEmit`, web build, map API smoke 18/18 with 0 failures, minimap smoke 0 failures with a historical preview-index warning later closed by the 2026-05-16 map audit, WS load 64/64 ready with 0 errors, and Stage 5 UI smoke 88 screenshots with 0 critical console errors all passed. The Stage 5 manifest checked 32 compact text nodes with no overflow. Frontend rows below remain Candidate/human-acceptance rows; R301 does not close the final Crystal visual/feel pass.
- 2026-04-28 R300 parity context: backend/server tracked-slice packet parity is now accepted under explicit stable-diff packet acceptance. Frontend rows below remain Candidate/human-acceptance rows; R300 does not close the final Crystal visual/feel pass.
- 2026-04-26 R225 regression refresh: `smoke:stage5-ui` still captures 88 screenshots and now writes manifest summary counts (8 compact panel bounds, 34 compact text nodes, 0 critical console errors, major flow counts). Direct `next build`, `tsc --noEmit`, map API smoke 18/18, minimap asset smoke 0 failures with a historical preview-index warning later closed by the 2026-05-16 map audit, WS load 64/64, Rust package regressions, `fmt --check`, and `diff --check` passed. The remaining frontend rows below are Candidate/human-acceptance rows, not unverified automatable gaps on this Mac.
- 2026-04-26 R224 integration evidence: `packet_trace --list-flows` works, `mir2-gateway` passes 53/53 including packet trace bin tests 6/6, and require-local `packet_trace --matrix` wrote 9 TCP-traceable artifacts under `docs/generated/packet-traces/r224-matrix` with `localOk=true`. Frontend/global automation remains **100% Candidate**; 100% Accepted still requires human Crystal visual/feel acceptance.
- 2026-04-26 R223 Candidate evidence: `smoke:stage5-ui` now captures 88 screenshots and records advanced Stage 5 systems state plus compact Mail/Report panel bounds. Direct `next build`, `tsc --noEmit`, map API smoke 18/18, minimap asset smoke 0 failures with a historical preview-index warning later closed by the 2026-05-16 map audit, WS load 64/64, full Rust package regressions, `fmt --check`, and `diff --check` passed.
- `npm.cmd run build`
- `npm.cmd run audit:crystal-map-coverage`
- `npm.cmd run smoke:crystal-minimap-assets`
- `npm.cmd run smoke:crystal-map-api`
- `npm.cmd run smoke:stage5-ui`
- `npm.cmd run load:gateway-ws`
- screenshot manifest: `docs/stage5-screenshots/stage5-ui-smoke-manifest.json`
- load evidence: `docs/generated/load/latest-ws.json`, `docs/generated/load/latest-tcp.json`
- map/API evidence: `docs/generated/map/latest-crystal-map-api.json`
- all-map source-resource audit evidence: `docs/generated/map/latest-crystal-map-coverage.json`
- minimap asset evidence: `docs/generated/assets/latest-minimap-assets.json`
- 2026-04-26 R184 evidence: direct `next build`, `smoke:crystal-minimap-assets`, `smoke:crystal-map-api`, `smoke:stage5-ui` (10 screenshots), and `load:gateway-ws` 64/64 ready passed locally on macOS with gateway on `127.0.0.1:7110`.
- 2026-04-26 R185 evidence: `smoke:stage5-ui` now captures 11 screenshots across desktop 1024x768 and compact 820x640 viewports, writes viewport metadata and compact layout bounds to `stage5-ui-smoke-manifest.json`, and includes `stage5-compact-game.png`.
- 2026-04-26 R186 evidence: `smoke:stage5-ui` now checks 33 visible compact text nodes for overflow, writes `compactTextLayout`, and the compact minimap title/Safe Zone label is fixed.
- 2026-04-26 R187 evidence: `smoke:stage5-ui` now captures 14 screenshots, exercises minimap collapse, BigMap re-expand, and Mail open paths, and writes `minimapFlow`.
- 2026-04-26 R188 evidence: `smoke:stage5-ui` now captures 17 screenshots, exercises belt rotate/close states, writes `beltFlow`, and asserts belt labels stay in-bounds without Quest overlap.
- 2026-04-26 R189 evidence: `smoke:stage5-ui` now captures 18 screenshots, presses belt hotkey `1`, verifies Red Potion quantity drops from 5 to 4, and writes `beltUseFlow`.
- 2026-04-26 R190 evidence: `smoke:stage5-ui` now captures 21 screenshots, switches inventory bag1/bag2/quest/bag1, and writes `inventoryFlow`.
- 2026-04-26 R191 evidence: `smoke:stage5-ui` now captures 25 screenshots, switches character char/stats1/stats2/spells/char, and writes `characterFlow`.
- 2026-04-26 R192 evidence: `smoke:stage5-ui` now captures 27 screenshots, switches storage page1/page2-locked/page1, and writes `storageFlow`.
- 2026-04-26 R193 evidence: `smoke:stage5-ui` now captures 31 screenshots, exercises chat Shout filter, All restore, Settings, collapse/restore, and Report paths, and writes `chatFlow`.
- 2026-04-26 R194 evidence: `smoke:stage5-ui` now captures 35 screenshots, opens the system menu, routes Character/Inventory/Quest actions, and writes `systemMenuFlow`.
- 2026-04-26 R195 evidence: `smoke:stage5-ui` now captures 36 screenshots, rents expanded storage from locked page 2, verifies unlocked page 2 plus 160-slot capacity, and writes the rented state into `storageFlow`.
- 2026-04-26 R196 evidence: `smoke:stage5-ui` now captures 37 screenshots, clicks Red Potion from inventory bag1, verifies quantity drops from 5 to 4, and writes `inventoryUseFlow`.
- 2026-04-26 R197 evidence: `smoke:stage5-ui` now captures 38 screenshots, clicks Dagger from inventory bag1, verifies it moves into the weapon equipment slot, and writes `inventoryEquipFlow`.
- 2026-04-26 R198 evidence: `smoke:stage5-ui` now captures 40 screenshots, opens Character Spells from HUD Skill and Stats II from HUD Option, and writes `hudButtonFlow`.
- 2026-04-26 R199 evidence: `smoke:stage5-ui` now captures 42 screenshots, opens Drop Gold, confirms 100 gold, verifies gold drops from 1280 to 1180 plus a ground-drop label, and writes `inventoryGoldFlow`.
- 2026-04-26 R200 evidence: `smoke:stage5-ui` now captures 43 screenshots, context-clicks Wooden Sword in bag1, verifies it moves from slot 4 to slot 10, and writes `inventoryMoveFlow`.
- 2026-04-26 R201 evidence: `smoke:stage5-ui` now captures 45 screenshots, opens Split Item for Red Potion, verifies the split stack lands in the belt while total Red Potion quantity is preserved, and writes `inventorySplitFlow`.
- 2026-04-26 R202 evidence: `smoke:stage5-ui` now captures 47 screenshots, opens Delete Item for Blue Potion, verifies quantity drops from 3 to 2 plus a ground-drop label, and writes `inventoryDropFlow`.
- 2026-04-26 R203 evidence: `smoke:stage5-ui` now captures 48 screenshots, verifies Character Dagger removal back to bag1 slot 4, fixes RemoveItem target/grid wiring, and writes `characterRemoveFlow`.
- 2026-04-26 R204 evidence: `smoke:stage5-ui` now captures 49 screenshots, clicks Red Potion directly in the belt, verifies quantity drops from 5 to 4 before hotkey `1` drops it from 4 to 3, and writes `beltMouseUseFlow`.
- 2026-04-26 R205 evidence: `smoke:stage5-ui` now captures 51 screenshots, opens Sell Item for Dagger, confirms without active sell service, verifies Dagger/gold are preserved, and writes `inventorySellFlow`.
- 2026-04-26 R206 evidence: `smoke:stage5-ui` now captures 54 screenshots, opens Store Item for Dagger, selects a warehouse slot without active storage service, verifies Dagger/storage contents are preserved, and writes `storageStoreFlow`.
- 2026-04-26 R207 evidence: `smoke:stage5-ui` now captures 57 screenshots, opens Take Back for stored Red Potion, selects an inventory slot without active storage service, verifies inventory/storage quantities are preserved, and writes `storageTakeBackFlow`.
- 2026-04-26 R208 evidence: `smoke:stage5-ui` now captures 58 screenshots, opens/closes Set Storage Password without submitting credentials, verifies panel state, and writes `storagePasswordFlow`.
- 2026-04-26 R209 evidence: `smoke:stage5-ui` now captures 60 screenshots, fills Set Storage Password, verifies mismatch disables submit, submits matching `Safe123` without active storage service, verifies no password is set with no-service feedback, and extends `storagePasswordFlow`.
- 2026-04-26 R210-R218 evidence: `smoke:stage5-ui` now captures 71 screenshots, records Mail/Report/NPC panel state, broad Stage 5 systems state, guild/group chat filters, Character repair/special-repair, ground item/gold pickup, combat target state, system-menu QA and transfer-list routing, Battle Focus spell casting, and compact inventory panel bounds.
- 2026-04-26 R219-R222 evidence: `smoke:stage5-ui` now captures 85 screenshots, records login/select lifecycle flows, compact inventory/storage/character/system-menu/chat-settings bounds, and existing broad gameplay/system flows. Map API smoke writes 18/18 successful requests, minimap asset smoke writes 0 failures with a historical preview-index warning later closed by the 2026-05-16 map audit, and WS load refresh reports 64/64 ready with 0 errors.
- 2026-04-26 R223 evidence: `smoke:stage5-ui` now captures 88 screenshots, records advanced Stage 5 systems state for trade item/cancel, shop gold purchase, auction buy/cancel, conquest end, hero behaviour, mining/craft, and mail delete state, and adds compact Mail/Report panel bounds.

## Open Gap Matrix

| Status | Area | Gap | Evidence Needed |
| --- | --- | --- | --- |
| [~] | Login/select | Language switching, View Key, Enter-key login submit, Credits, Delete cancel, New Character, confirmed Delete Character, recreate, slot selection, Start, login music, login effect, and select music are implemented or smoke-verified; pixel/audio comparison against Crystal login/select screens still open | screenshots and human acceptance |
| [~] | Game shell | First viewport now has desktop/compact automated route screenshots; R303 confirms all 463 manifest map files are source-present/parseable for sampled frontend loading; R304 proves the aligned Bichon web runtime snapshot includes current-map Crystal NPCs; R305 proves the same view includes first-pass visible respawns; R306 removes the default quest tracker overlay while normalizing visible NPC/monster nameplates; R307 proves the second Bichon comparison point includes ordinary Guard plus ArcherGuard; R308 removes browser-only original-size stage downscaling/frame decoration while exporting the missing Bichon NPC/animal sprite libs; R309 removes the measured desktop minimap 2px overflow; R310 clears the login transition before game capture while removing over-broad NPC quest markers; R311 adds Crystal bitmap HUD orb fills; R312 restores Crystal source projection anchors for entity sprites/nameplates/health bars; the 2026-05-08 pass aligns Bichon torch/fire blend frames back onto the red torch head; the 2026-05-14 full-resource sync closes minimap asset coverage; and the 2026-05-16 all-map audit closes automated source/fallback risk with Crystal no-draw frame classification. Remaining open items are exact dynamic density/placement and human Crystal-like visual judgment | screenshot comparison at accepted viewports |
| [~] | HUD/chat | Crystal chat control-bar semantics are now smoke-verified: All/Shout/Whisper/Lover/Mentor/Group/Guild set outgoing prefixes, Trade sends `tradeRequest`, Settings owns channel visibility plus transparency, Size collapses/restores, Report opens/closes, hit boxes are 24x13 and topmost over the HUD; latest-line auto-follow, scroll-knob behavior, 4-line Crystal chat feed, and bitmap HP/MP orb fills are also implemented or verified. Remaining panel-level acceptance is Crystal visual/feel comparison, especially exact text placement and HUD/chat feel | targeted chat-control smoke, UI smoke/capture passed; human pass remains |
| [~] | Belt | Slots 1-6, rotate/close, occupied/empty visuals, in-bounds labels, no Quest overlap, mouse Red Potion use, and hotkeys `1..6` are smoke-verified; consumable hotkeys decrement and empty/non-consumable slots are recorded as no-op coverage. Full Crystal feel remains open | automated command path plus human pass |
| [~] | Minimap | Compact map title/Safe Zone text no longer overflows, collapse/BigMap re-expand/Mail open paths are smoke-verified, and the 2026-05-14 full-resource sync exports minimap ids 450/451 from Crystal database preview BMPs; direct Crystal visual comparison remains open | smoke plus screenshot comparison |
| [~] | Inventory | bag1/bag2/quest tabs, Red Potion item use/split, Blue Potion item drop, Dagger equip/remove, Sell Item no-service preserve, service-backed Store Item, service-backed Take Back, Drop Gold, and Wooden Sword move are smoke-verified with screenshots and state evidence; item merge/full service-backed sell and deeper panel acceptance remain open | UI route plus backend packets |
| [~] | Character | char/stats1/stats2/spells tabs, known skill display, HUD Skill/Option button routes, Battle Focus cast/buff/cooldown, Dagger equipment remove, equipped-slot normal repair through Blacksmith_Smith `@Repair`, equipped-slot special repair through Blacksmith_Bill `@SRepair`, durability restoration/max-loss behavior, gold deduction, and corrected Belt/Boots equipment-slot ids are smoke/test-verified; deeper paperdoll and durability UI feel acceptance remains open | screenshot plus interaction route |
| [~] | Combat/skills/effects | Battle Focus cast/buff/cooldown, targeted combat, live Crystal magic packets, projectile packets, buff add/remove/pause deltas, map-effect fallback, and Crystal-like visual action timing are smoke/type-verified; remaining work is full per-class skill visual/effect fidelity and human feel comparison against Crystal | packet/event smoke plus human visual pass |
| [~] | NPC/shop/storage | storage page 1, locked expanded page 2, expanded-storage rent/unlock, restored page 1, InnKeeper_Brittney `@Storage` service open through the real Crystal dialog path, service-backed Dagger store, service-backed Red Potion take-back, Blacksmith `@Repair` / `@SRepair` service-backed repairs, GameShop Gold buy with carry-slot delivery, NPC dialog markup sanitization, and dialog link rendering are smoke/build-verified; input, sell/craft/refine panels and GameShop preview still need Crystal comparison | route screenshots and packet trace |
| [~] | Storage password | expanded storage confirmation, Set Storage Password panel entry, mismatch validation, and service-backed set/unlock/change/remove password flows are smoke-verified through InnKeeper_Brittney `@Storage`, including last-set timestamp exposure; exact Crystal dialog bitmap and invalid-password edge visual acceptance remain open | UI route and persistence check |
| [~] | Quest/mail/report/menu | Mail open/close state, real Mail claim for seeded GameShop gold, nested-button-free Mail row DOM, Report open/close state, compact Mail/Report bounds, system menu QA Jump, transfer-list routing, trade chat filtering, dynamic social panels, Quest Diary title/stage/progress/reward rows, and repo-stable Stage 5 smoke screenshots are verified. R316 replaces the visible Web debug menu with Crystal `MenuDialog` frame `Title/567` and icon buttons, and Gameshop now opens the Crystal-framed dialog instead of Inventory/Quest. R317 replaces Gameshop placeholder cells with Crystal product manifest data, original cell/buttons, item icons, categories, pagination, prices, stock, and payment controls. Quest/mail/report exact dialog bitmaps and service-backed Gameshop preview behavior still need Crystal-like layout and interaction review | screenshot and human pass |
| [~] | Scene interaction | tile buttons avoid scene pointer double-dispatch and now track the same camera motion offset as rendered map/entities; added-stat ground drops render with server-provided Crystal Cyan name colour; selected scene targets route keyboard approach/primary actions; Blue Potion and gold ground pickup plus combat target selection are smoke-verified; R321 can now control original Crystal and Web for movement comparison; R322 prevents prediction/server-correction movement loops by making prediction visual-only and stopping plans on server correction; R323 adds held-mouse 100ms input sampling with direct `Walk` / `Run` direction packets for Crystal-like queued movement; R325 fixes held-run visual background backtracking by synchronizing predicted render basis and motion snapshots; R326 keeps the latest held-direction request queued during the 600ms action window and consumes it like Crystal `QueuedAction`; the 2026-05-07 movement/animation pass anchors player/monster/NPC frame cycles to Crystal action timing; the 2026-05-08 movement pass ties displacement to Crystal six-frame/even-pixel `OffSetMove` cadence and caps held-direction prediction to one unconfirmed action; the 2026-05-09 direction/queue pass carries predicted facing with predicted coordinates, keeps target prediction until the server reaches the target, and avoids click-route rollback in headless captures; the 2026-05-22 production pass clears confirmed ACK actions from the local movement feed and keeps the motion clock live under headless throttling; the 2026-05-23 production Crystal action-queue pass drives self Walk/Run/Turn through local `QueuedAction`/ActionFeed ACK/correction semantics with strict production walk/run evidence green. Remaining movement acceptance is final human feel plus broader blocked-tile/collision edge parity | route replay and human pass |
| [~] | Responsive/layout | R308 records exact 1024x768 stage bounds at original comparison size and keeps the 820x640 compact stage inside viewport bounds; R309 records minimap/core HUD bounds with no desktop or compact overflow; compact inventory/storage/character/system-menu/chat-settings/Mail/Report/social bounds are smoke-verified; the current compact matrix covers 900x640, 768x640, and 820x540. The 2026-05-19 mobile landscape pass adds nipplejs joystick controls, Mir2 direction/run semantic mapping, a right-bottom circular action wheel, a portrait rotate prompt, and strict mobile movement smoke evidence. The 2026-05-22 production pass dynamically scales the 1024x768 stage to the actual viewport and verifies a 150x647 DevTools-width login/game path with stage bounds inside the viewport. Broader human mobile feel, especially sustained high-frequency run/turn behavior, remains open | screenshot checks |
| [~] | Language/text | Compact visible core quest/HUD/minimap/belt/chat/entity/mail/storage/system/social text is smoke-checked with overflow-safe selectors and CSS; login/select language switches remain smoke-covered, while full language-by-panel visual acceptance remains open | screenshot and DOM checks |

Candidate note: as of R301, all rows above have automated evidence for the available route. They intentionally remain `[~]` until direct Crystal screenshots/live comparisons or human visual/feel acceptance close them; automation should not flip them to `[x]` by itself.

## Recent Frontend Fixes

- 2026-07-13: Crystal asset semantic-source phase 1 now preserves `.Lib` v3 `FrameSet` records instead of skipping the header seek. The shared parser exposes all original action fields (`Start`, `Count`, `Skip`, `Interval`, effect fields, `Reverse`, and `Blend`), validates truncated/corrupt ranges, and the UI exporter now writes this FrameSet into per-library and aggregate metadata while selective exports merge existing libraries instead of erasing the manifest. A deterministic Source Snapshot streams SHA-256 over the full local Crystal Data tree without decoding image payloads. Full-source evidence at `docs/generated/assets/crystal-source-snapshot.generated.json`: 1,440/1,440 libraries parsed, 7,638,253,548 source bytes, 2,143,132 frame slots, 585 v2 plus 855 v3 libraries, 703 non-empty FrameSets, 3,643 actions, and zero parse failures, invalid offsets, unknown actions, duplicate actions, or reported issues; two consecutive full generations produced byte-identical file SHA-256 `C3480F6689CF27C3CECC81ED86787BEBF5283B089B58644529E76E4AA09197F9`. Focused `test:crystal-library`, legacy `test:magic-effect-export`, syntax checks, synthetic selective-export merge, and a real `NPC/00.Lib` export passed. This closes source/metadata preservation only; Bevy runtime consumption of generated FrameSets, masks, and effect recipes remains open.
- 2026-05-24: Production movement visual closeout deployed as Player Web `dpl_8wQigG43KBLpaZY5oPPWHwNhz3QK`. Rapid discrete keyboard taps now carry a bounded same-direction input debt across server ACK latency, so six quick D taps send six walk packets and receive six ordered `UserLocation` ACKs instead of collapsing to two steps. The original-map renderer now keeps a textured floor fallback under still-loading map tiles and alpha-keys black-background map Object images before showing them, preventing the black rectangle flash that made movement feel broken even after the packet path was correct. Evidence: Web `pnpm --dir apps/web exec tsc --noEmit --pretty false`; scoped diff check; Vercel prebuilt build/prune/deploy; custom-domain `/health`; production `docs/generated/player-qa/movement-jitter/prod-underlay-keyboard-d-20260524T112642.json`; and headed Chrome `docs/generated/player-qa/movement-jitter/prod-underlay-headed-keyboard-d-20260524T112744.json`, both `ok=true` with no visual jumps, no route spam, no logical rollback, no scene blackouts, no console errors, no non-favicon 404s, and screenshot evidence without black map holes.
- 2026-05-23: Crystal action-queue movement pass landed across Web/Gateway/Simulation and was production-verified. Player Web now treats self `UserLocation` as an ordered ACK/correction surface for local `QueuedAction`/ActionFeed state, not as a fresh walk/run animation; correction snaps clear packet motion, same-tile confirmations preserve the active animation, and server `UserLocation` no longer re-seeds predicted motion when it is only confirming the local queue. Packet Walk/Run rendering now uses one Crystal 600ms action window even for two-tile Run, matching the original sprite cadence instead of stretching Run over two tiles. Backend Zone consumes bounded ordered Walk/Run/Turn actions on Crystal `ActionTime`; the later local rollback correction changes raw standstill Run from an origin correction into an effective one-tile Walk. The production follow-up capped local ActionFeed lead to two tiles and treats non-matching `UserLocation` as correction, closing the residual visual jump found after the first deploy. Evidence: Web `pnpm --dir apps/web exec tsc --noEmit --pretty false`; Web `pnpm --dir apps/web exec next build`; Simulation/Gateway fmt-check; Simulation `shared_zone` 78/78; focused Gateway Walk+Run/Turn regressions; local captures `docs/generated/player-qa/movement-jitter/crystal-action-queue-local-shiftd-20260523.json` and `docs/generated/player-qa/movement-jitter/crystal-action-queue-local-da2-20260523.json`; action-queue verification deployment `dpl_HmHQ4CXfy7d895kHFMfiNLHWespN`; and production captures `docs/generated/player-qa/movement-jitter/prod-action-queue-keyboard-walk-fix2-20260523T1331.json` plus `docs/generated/player-qa/movement-jitter/prod-action-queue-keyboard-run-fix2-20260523T1332.json`, both `ok=true` with no visual jumps, logical rollback, scene blackouts, critical console errors, or non-favicon 404s.
- 2026-05-22: Production movement/layout hardening landed for the user-reported Chrome/DevTools failures. The original 1024x768 client stage now uses a viewport-driven CSS scale so very narrow DevTools layouts keep login/select/game controls inside the visible viewport; `/health` is a no-store Next route instead of a Vercel 404; hydration risk is reduced with deterministic initial motion state plus layout-level hydration suppression; and self movement ACKs now confirm local pending/fed actions even when the optimistic client state already matches the incoming `UserLocation`, pruning `outstandingSelfMovementActions` after visual settlement. A requestAnimationFrame plus 100ms fallback keeps `motionNow` live in headless/throttled Chrome. Production deployment `dpl_Gr9WgZX275rpfDfk9f4SdzAshogb` is live behind `https://mir2.obelisk.build/`; custom-domain `/health` returned 200, and `Monster/000/51.png` returned 200. Evidence: `pnpm --dir apps/web exec tsc --noEmit --pretty false`; `pnpm --dir apps/web exec next build`; `docs/generated/player-qa/movement-jitter/prod-final-narrow-stage-scale-20260522.json` (`ok=true`, 150x647 stage bounds `left=-0.01`, `width=150.02`, no console errors, no non-favicon 404s, no residual movement queues); and `docs/generated/player-qa/movement-jitter/prod-final-movement-ack-prune-skip-transfer-20260522.json` (`ok=true`, strict movement checks green, `directionStepPending=null`, `outstandingSelfMovementActions=[]`, no visual jumps, no route spam, no logical rollback, no camera-offset stair-step warnings). The failed `prod-final-movement-ack-prune-20260522` run is expected evidence that production correctly rejects debug `crystal:<map>:<x>:<y>` transfer commands for normal clients.
- 2026-05-20: Production self-movement rendering now preserves local/packet movement animation state even when the authoritative player tile already equals the predicted tile, so the self player no longer drops back to a standing frame during walk/run confirmation. Camera/entity motion offsets now keep fractional pixels instead of truncating every frame to integer pixels, removing the stair-step feel during tile interpolation. The movement QA harness also records self movement animation fields and retries an explicit navigation when Chrome creates a target before hydration completes. Evidence: production Vercel deployment `dpl_ArWKGQbfwi5F3viVUsNsoumktTuD`; web `pnpm --dir apps/web exec tsc --noEmit --pretty false`; `node --check apps/web/scripts/capture-web-movement-jitter.mjs`; production headed capture `docs/generated/player-qa/movement-jitter/prod-movement-animation-fix-20260520-click-target.json` with `ok=true`, `noVisualJumps=true`, `cameraOffsetMovesContinuously=true`, `directionAnimationWithinCrystalWindow=true`, `noConsoleErrors=true`, `noNonFaviconNetwork404s=true`, `97` samples, `65` fractional-offset samples, and live `walk/run` packets through `packetRefresh`. A focused packet-run probe `prod-movement-animation-fix-20260520-packet-run-left-rerun.json` additionally captured `movementAnimation="running"` on self samples and running motion snapshots after real `UserLocation` packets; its overall `ok=false` is expected because the probe deliberately sends repeated packetRun commands and trips spam/direction-window assertions.
- 2026-05-16: MiniMapDialog crop/title/collapse alignment now follows the Crystal source frame more closely. The 120x108 minimap viewport now crops the full minimap raster with Crystal-style negative source offsets instead of scaling a cropped image over a transparent frame, preventing the main scene from leaking into the top-right minimap. The large minimap title now renders one centered map-name line without appending Safe Zone, collapsed mode switches to the small `Prguse/2091` frame and hides title/scene content, mini-map button hit boxes remain stable in both expanded/collapsed modes, and radar colors match Crystal player/NPC/monster dots. A dedicated minimap-only Stage 5 smoke now covers expanded/collapsed/BigMap/Mail states without running the whole 100+ screenshot suite, and the movement prediction state update no longer emits React `flushSync` lifecycle warnings during smoke login/bootstrap. Evidence: Web `node --check scripts/smoke-stage5-ui.mjs`, `npx tsc --noEmit`, direct `npx next build`, and live `MIR2_STAGE5_SMOKE_MINIMAP_ONLY=1 MIR2_WEB_BASE_URL=http://127.0.0.1:13010/?gatewayWs=ws://127.0.0.1:7210/ws node scripts/smoke-stage5-ui.mjs`, which wrote `docs/stage5-screenshots/stage5-minimap-smoke-manifest.json` with `mode="minimap-only"`, 17 screenshots, `criticalConsoleErrors=[]`, expanded `nameText="BichonProvince"`, `titleCount=1`, `sceneHidden=false`, `sceneHasRaster=true`, `sceneHasFallback=false`, collapsed `titleCount=0`, `sceneHidden=true`, `smallMode=true`, BigMap/Mail open states, and all minimap button hit tests uncovered.
- 2026-05-15: Closed the remaining high-frequency movement residual/rollback path by committing completed local self movement into the client-side CurrentLocation and shielding it from stale snapshot echoes. Verification evidence is `r-highfreq-keyseq-da-after-local-current-location-16ms.json`, `r-long-shiftd-after-local-current-location-16ms.json`, and `r-right-left-after-local-current-location-16ms.json`, all strict-green with no visual jumps, no logical rollback, no queue residue, and no browser errors.
- 2026-05-15: Closed the chat control-bar button loop. The row beneath the belt now follows Crystal `ChatControlBar`: channel buttons set outgoing prefixes, Trade dispatches `tradeRequest`, Settings controls visible chat channels and transparency, Size collapses/expands, and Report opens/closes. A dedicated smoke uses real mouse events and hit-testing to prove the controls are not covered by the HUD: `docs/stage5-screenshots/stage5-chat-controls-smoke-manifest.json` records every button `topMatches=true`, all prefixes verified, chat send with `!Codex shout smoke`, trade command dispatch, settings toggles, report open/close, and zero critical console errors.
- 2026-05-11: Movement input-latency investigation matched Crystal's client/server loop more closely: Web movement confirmation ticks now run only while movement is busy, target-click plans preserve the local action anchor when switching from held direction input, early same-tile `UserLocation` echoes are retried instead of treated as immediate rollback, and route corrections use the originally sent direction when deciding whether to hold predicted motion. The simulation now queues one over-early Walk/Run retry for the next world tick, matching Crystal's `_retryList` behavior instead of dropping the packet. Evidence: web `npx tsc --noEmit`, `cargo +1.89.0 fmt --check -p mir2-simulation -p mir2-gateway`, focused simulation `crystal_packet_walk_timing_rejects_repeat_until_world_tick_advances`, and captures `r-manual-jitter-fix-0511j` / `r-keyboard-after-retry-0511a`. Remaining stress failures are blocked/dynamic-entity cases where the client still predicts into a tile the server later rejects; those now avoid visual rollback but still need collision/blocked-target feel parity.
- 2026-05-09: Original-client frontend shell was split into smaller ownership modules instead of keeping the Crystal client in one multi-thousand-line component. `original-client-shell.tsx` is now the input/state orchestrator (970 lines), with shared display contracts, shell flow constants/props, HUD/window composition, scene layout, scene motion timing, scene map rendering, scene sprite rendering, visual layers, inventory action/password panels, and social-system definitions split under `apps/web/app/components/`. Evidence: web `tsc --noEmit`, `next build`, `git diff --check`, live 13010 HTTP 200 after dev-server restart, `r-final-component-split-input-234609.json` (`jumps=[]`, held run 330,270 -> 336,270), `r-final-component-split-route-234706.json` (`jumps=[]` for route replay), and `r-final-component-split-mirguide-click-235004.json` (`dialogTitle="MirGuide_Peter"`, zero console errors).
- 2026-05-09: NPC quest markers now anchor to the rendered NPC sprite center instead of the tile fallback/name offset, with stable 28x29 Crystal icon dimensions and NPC sprite/name hitboxes using the real frame bounds. Closing an NPC dialog now sends `@Exit`, so the next click can reopen the same NPC instead of staying hidden behind a stale dismissed dialog key. Evidence: web `npx tsc --noEmit`, live Gateway WS new-account/new-character `Village Guide` interact/`@Exit` probe, and headless DOM click probe opening/closing `Village Guide` from the sprite hitbox with zero console errors.
- 2026-05-09: Movement prediction now includes facing direction, so local run/walk prediction turns the sprite in the same frame as the predicted tile. Target-click movement no longer clears the local target prediction while the server is still confirming an earlier tile, and queued move dispatch starts 50ms before the 600ms visual action boundary to reduce input-lag feel without changing the frame animation length. Evidence: web `npx tsc --noEmit`; `r-direction-queue-after.json` with held-run `jumps=[]`; `r-click-direction-after-restart.json` with right-click target prediction held at `338,270` while the server was still at `336,270`, `direction="Right"`, and `jumps=[]`; `r-route-direction-after.json` with route replay `jumps=[]`.
- 2026-05-08: Movement feel cleanup aligns Web walk/run displacement with Crystal's frame-driven `OffSetMove`: camera/entity offsets advance on the six 100ms movement frames, snap to even integer pixels, and held-direction prediction now waits for the prior action to be confirmed or timed out before adding another local run/walk step. Evidence: `r-movement-direction-pending-cleared-after.json` and `r-movement-click-direct-crystal-frame-after.json`, both with `jumps=[]`, no browser errors, and no non-favicon 404s.
- 2026-05-07: Closed the frontend 2/4/5/6 automation slice. Live Crystal magic/projectile/buff packets now update Web combat visuals and skill/buff state; late-system System Menu social panels include trade/market/marriage plus state-backed rows/actions; NPC/quest smoke uses real Crystal dialog links without QA storage fallback and strips Crystal script markup; compact smoke now runs a three-viewport matrix with repo-stable screenshot output. Evidence: 113-screenshot Stage 5 UI smoke, `criticalConsoleErrorCount=0`, `compactMatrixCount=3`, `systemMenuSocial=36`, `npcDialogFlow=11`, and `combatFlow=2`.
- 2026-05-07: Entity movement and idle animation now follow Crystal timing more closely. Walk/run packets create a 600ms action window for sprite motion, walking/running frames start from the packet time, standing player/monster/NPC frames animate at their Crystal idle intervals, and the extra CSS attack hop was removed. Evidence: `docs/generated/player-qa/movement-jitter/r-movement-animation-crystal-timing.json` / `.png` and the live-tick companion capture.
- 2026-05-07: Player Web gateway targeting is no longer hardcoded for test harnesses: `?gatewayWs=ws://host/ws` or `NEXT_PUBLIC_MIR2_GATEWAY_WS_URL` can point the client at an isolated Gateway, while the default remains `ws://127.0.0.1:7110/ws`.
- 2026-05-07: Stage 5 UI smoke now activates InnKeeper_Brittney's Crystal `@Storage` service, then verifies service-backed storage password set/unlock/change/remove, Dagger store, and Red Potion take-back. The refreshed manifest captures 101 screenshots and reports 0 critical console errors.
- 2026-05-07: Equipped-item normal/special repair is now backed by real Crystal NPC services. The smoke damages equipped Dagger, repairs through Blacksmith_Smith `@Repair` and Blacksmith_Bill `@SRepair`, and verifies durability/gold mutations; focused simulation regressions cover equipped-slot repair ids and the QA damage setup.
- 2026-05-07: GameShop now has a positive Gold-purchase smoke path. The script seeds Gold through real Mail claim, buys `AccuracyPotion`, verifies the Gold deduction, carry-slot delivery, and purchase chat feedback, then archives `stage5-gameshop-gold-open.png` and `stage5-gameshop-gold-buy.png`.
- 2026-05-07: Mail rows no longer nest action buttons inside a row button. The Crystal-style row is now `role="button"` with keyboard activation, leaving claim/delete as real child buttons and keeping full smoke `criticalConsoleErrorCount=0`.
- 2026-05-07: Stage 5 UI smoke now presses belt hotkeys `1..6`, requiring Red/Blue Potion slots to decrement and recording empty or non-consumable occupied slots as no-op coverage instead of leaving broader hotkeys untested.
- 2026-05-01: R326 held-mouse queued-action fix keeps the latest 100ms held-pointer direction request instead of returning during the current 600ms walk/run action window. This matches Crystal's `QueuedAction` overwrite/consume behavior while preserving one movement packet per completed action. Evidence: `docs/generated/player-qa/movement-jitter/r326-web-hold-run-queued-direction.json` final `344,270`, `movementPlan=null`, `jumps=[]`, `r326-web-hold-run-ws-send-probe.json` with repeated WebSocket `run Right` entries, plus gateway move logs showing `Run Right` through `344,270`; web `tsc --noEmit` passed.
- 2026-05-01: R327 wires Gameshop Buy to manifest-backed backend purchase commands and fixes right-click map-click arrival. Evidence: `r327-gameshop-buy-click-final-clean-state.json` sends `gameShop.buyCredit(20,1)` with the expected zero-credit rejection and no browser 404/errors; `r327-map-click-target-arrival-fixed3.json` reaches `338,270` with `movementPlan=null` and `jumps=[]`. Verified by web typecheck, script syntax checks, focused simulation game-shop test, gateway check, and CDP captures.
- 2026-05-01: R325 held-run visual jitter fix keeps predicted movement in the render basis and refreshes motion snapshots synchronously before paint, preventing map/camera backtracking when service snapshots confirm earlier tiles. Evidence: `docs/generated/player-qa/movement-jitter/r325-web-hold-run-final-4s.json` records final `344,270`, `movementPlan=null`, fixed map sprite continuity, and `jumps=[]`; verified with web `tsc --noEmit`, capture-script syntax check, `mir2-gateway` check, and targeted movement captures.
- 2026-05-01: R322 movement correction cleanup removes predicted coordinates from the logical movement source and clears the movement plan when the server corrects to a different tile. This fixes the severe back-and-forth loop around blocked or partially blocked tiles. Evidence: `docs/generated/player-qa/movement-jitter/r322-web-movement-correction-stop.json` and `r322-web-movement-open-area.json`, both with `jumps=[]`; verified with web `tsc --noEmit`.
- 2026-05-01: R323 held-mouse movement now samples held scene input every 100ms and sends Crystal-like `Walk` / `Run` direction packets for queued movement instead of feeding absolute `moveTo` targets. Evidence: `r323-web-hold-run-direct-direction.json` final `340,270`, `r323-web-hold-walk-direct-direction.json` final `335,270`, and `r323-web-packet-run-right.json` final `340,270`; all have `jumps=[]`. The later R327 asset export removed the missing `original-ui/NPC/25/meta.json` warning from this scene.
- 2026-05-01: R321 movement-control diagnostics add original-client Win32 mouse/screenshot automation and richer Web movement sampling. Evidence at `docs/generated/player-qa/movement-jitter/r321-web-movement-direct.json`, `r321-web-movement-click-actions.json`, `r321-web-movement-click-hitoffset.json`, and `r321-original-movement-control.json` separates backend traversal from input feel. Direct `moveTo` advances each step cleanly with no jumps, and moving the transparent tile hit layer with `playerCameraMotionOffset` fixes the initial click/right-click hit-test delay; remaining movement feel work is Crystal-like continuous queued input handling.
- 2026-04-30: R317 Gameshop product-grid cleanup replaces placeholder cells with the generated Crystal Gameshop manifest, original `Title/750` item-cell frame, buy/preview button sprites, real item icons, categories, pagination, stock/count/price labels, and gold/credit payment controls. Evidence at `docs/generated/player-qa/r317-gameshop-products/r317-gameshop-products-state.json` records 8 visible cells, `pageLabel="1 / 14"`, `loadedIconCount=8`, zero placeholder cells, zero non-favicon 404s, and zero console errors.
- 2026-04-30: R316 Gameshop/Menu cleanup fixes the HUD Gameshop miswire from Inventory/Quest to a Crystal-framed Gameshop shell and replaces the visible large Web menu/debug panel with the 36x282 Crystal `MenuDialog` icon strip. Evidence at `docs/generated/player-qa/r316-gameshop-menu/r316-gameshop-menu-state.json` records `shopVisible=true`, `inventoryVisible=false`, `menuBounds=988,349,36,282`, `iconCount=13`, no old overlay header, zero non-favicon 404s, and zero console errors.
- 2026-04-30: R312 entity projection/nameplate alignment restores Crystal source `MapControl.OffSetY` math and moves entity sprite/nameplate/health anchors to Crystal `DrawLocation` / `DisplayRectangle` placement. Evidence at `docs/generated/player-qa/r312-entity-crystal-anchor/r312-bichon-287-618-entity-anchor-state.json` records `QA0429Hero` nameplate `top=275`, exact stage/HUD bounds, zero non-favicon 404s, and zero console errors.
- 2026-04-30: R311 playfield camera/HUD orb cleanup centers the Web map viewport on the Crystal playfield height above the HUD and replaces CSS-gradient HP/MP orb fills with Crystal `Prguse` frame 4 bitmap slices. R311 evidence at `docs/generated/player-qa/r311-playfield-camera/r311-bichon-287-618-hud-orb-state.json` records `QA0429Hero` nameplate `top=325`, exact stage/HUD bounds, zero non-favicon 404s, and zero console errors.
- 2026-04-29: R310 same-scene visual-watch bootstrap fixes the login transition overlay leaking into game screenshots, scopes NPC quest icons by server-provided `questIds`, adds `capture-crystal-parity.mjs` for deterministic Web game captures, and adds `r310-visual-watch.ps1` for original/Web long-run sampling. R310 evidence at `docs/generated/player-qa/r310-visual-watch/r310-final-web-scene-state.json` records `transitionOverlayVisible=false`, `questMarkerCount=0`, exact 1024x768 stage/HUD bounds, zero non-favicon 404s, and zero console errors.
- 2026-04-29: Bichon minimap/HUD bounds cleanup moves `.mini-map-panel` from `right=-2px` to `right=0`, closing the measured desktop `right=1026` overflow. R309 evidence at `docs/generated/player-qa/r309-minimap-bounds-web-page-state.json` records desktop minimap `right=1024`, empty overflow arrays, zero non-favicon 404s, and zero console errors.
- 2026-04-29: Bichon viewport/resource cleanup removes the 0.9 desktop downscale, removes decorative page/frame background effects, keeps compact-only 0.78 scaling, and exports missing `NPC/00`, `NPC/01`, `NPC/03`, `NPC/11`, `NPC/15`, `Monster/003`, `Monster/004`, and `Monster/005` sprite libraries. R308 evidence at `docs/generated/player-qa/r308-stage-scale-web-page-state.json` records exact desktop stage bounds, compact bounds, zero non-favicon 404s, and zero console errors.
- 2026-04-29: Bichon `0:287,618` guard/archer evidence now has a focused simulation regression and browser capture. R307 evidence at `docs/generated/player-qa/r307-bichon-guard-archer-web-page.png` and `docs/generated/player-qa/r307-bichon-guard-archer-web-page-state.json` records `hasGuard=true`, `hasArcherGuard=true`, `hasUnderscoreNameplate=false`, and `questTrackerVisible=false`.
- 2026-04-29: Bichon same-scene display cleanup removes the default web quest tracker overlay from the game playfield and displays NPC/monster nameplates without underscores. R306 evidence at `docs/generated/player-qa/r306-bichon-display-web-page.png` and `docs/generated/player-qa/r306-bichon-display-web-page-state.json` keeps 8 NPC sprite elements plus 8 monster sprite elements and records `hasUnderscoreNameplate=false`, `questTrackerVisible=false`.
- 2026-04-29: Bichon same-scene visible respawns now enter the world snapshot and page state. R305 evidence at `docs/generated/player-qa/r305-bichon-visible-world-snapshot.json`, `docs/generated/player-qa/r305-bichon-visible-web-page.png`, and `docs/generated/player-qa/r305-bichon-visible-web-page-state.json` shows 8 NPC sprite elements plus 8 monster sprite elements, including Deer and Royal_Guard.
- 2026-04-29: Bichon same-scene runtime population now includes current-map Crystal NPC-info manifest entries on saved-character start and transfer. Live WS evidence at `docs/generated/player-qa/r304-bichon-npc-world-snapshot.json` shows 8 NPCs around `0:284,607`; browser evidence at `docs/generated/player-qa/r304-bichon-npc-web-page.png` and `docs/generated/player-qa/r304-bichon-npc-web-page-state.json` shows 8 NPC sprite elements rendered in page state. Focused/adjacent simulation tests, gateway build, live WS probe, and browser capture passed.
- 2026-04-22: `LoginOverlay` account/password inputs now submit on Enter through the existing login handler; scene tile hit buttons now mark themselves UI-interactive and stop pointer bubbling so tile actions are handled once while empty-space scene clicks remain available. `npm.cmd run build --prefix E:\mir2\mir2-web3\apps\web` passed.
- 2026-04-22: Ground-drop labels now preserve and render server `nameColourArgb`, including Crystal Cyan for added-stat item drops. `npm.cmd run build --prefix apps\web` passed.
- 2026-04-22: Selected scene targets now expose localized action/distance nameplate feedback and keyboard approach/primary-action routing through the existing target handlers. `npm.cmd run build --prefix apps\web` passed.
- 2026-04-26: Chat now opens on the newest filtered lines, follows new messages while at the bottom, preserves scrollback when the user scrolls up, and moves the Crystal scroll knob with position. Headless/no-WebGL UI smoke now stays in DOM mode, Crystal map API locally falls back to packaged starter-region data when Crystal map files are absent, and Stage 5 UI smoke detects macOS Chrome. Direct `next build`, map/minimap smokes, Stage 5 UI smoke, and WS load passed.
- 2026-04-26: Stage 5 UI smoke now archives named desktop and compact viewport evidence, captures `stage5-compact-game.png`, and asserts compact core UI bounds before writing the screenshot manifest.
- 2026-04-26: Stage 5 UI smoke now asserts visible compact core text does not overflow. The new check found and fixed compact minimap title wrapping by splitting the map title and Safe Zone label into a stable two-line header.
- 2026-04-26: Stage 5 UI smoke now clicks minimap collapse, BigMap re-expand, and Mail open paths, archives three minimap screenshots, and records `minimapFlow` state.
- 2026-04-26: Stage 5 UI smoke now rotates and closes the belt, archives three belt screenshots, records `beltFlow`, and checks that slot labels remain inside the belt and the vertical belt does not overlap Quest.
- 2026-04-26: Stage 5 UI smoke now presses belt hotkey `1`, verifies Red Potion quantity decreases, archives `stage5-belt-hotkey-use.png`, and records `beltUseFlow`.
- 2026-04-26: Stage 5 UI smoke now switches inventory bag1, bag2, quest, and back to bag1, archives three tab screenshots, and records `inventoryFlow`.
- 2026-04-26: Stage 5 UI smoke now switches character char, stats1, stats2, spells, and back to char, archives four tab screenshots, and records `characterFlow`.
- 2026-04-26: Stage 5 UI smoke now switches storage page 1, locked expanded page 2, and back to page 1, archives two page-state screenshots, and records `storageFlow`.
- 2026-04-26: Stage 5 UI smoke now exercises chat Shout filter, All restore, Settings, collapse/restore size, and Report paths, archives four chat-control screenshots, and records `chatFlow`.
- 2026-04-26: Stage 5 UI smoke now opens the system menu, verifies transfer/action labels, routes Character, Inventory, and Quest actions, archives four system-menu screenshots, and records `systemMenuFlow`.
- 2026-04-26: Stage 5 UI smoke now rents expanded storage from locked page 2, verifies page 2 unlocks with expanded capacity/expiry copy, archives `stage5-storage-page2-rented.png`, and records the rented state in `storageFlow`.
- 2026-04-26: Stage 5 UI smoke now clicks Red Potion from inventory bag1, verifies quantity drops from 5 to 4, archives `stage5-inventory-use-red-potion.png`, and records `inventoryUseFlow`.
- 2026-04-26: Stage 5 UI smoke now clicks Dagger from inventory bag1, verifies it moves into the weapon equipment slot, archives `stage5-inventory-equip-dagger.png`, and records `inventoryEquipFlow`.
- 2026-04-26: Stage 5 UI smoke now routes HUD Skill to Character Spells and HUD Option to Stats II, archives two HUD-button screenshots, and records `hudButtonFlow`.
- 2026-04-26: Stage 5 UI smoke now opens Drop Gold, confirms 100 gold, verifies gold decreases and a ground-drop label appears, archives two gold-drop screenshots, records `inventoryGoldFlow`, and fixes missing `ui.confirm` fallback text.
- 2026-04-26: Stage 5 UI smoke now context-clicks Wooden Sword in bag1, moves it from slot 4 to slot 10, archives `stage5-inventory-move-wooden-sword.png`, and records `inventoryMoveFlow`.
- 2026-04-26: Stage 5 UI smoke now opens Split Item for Red Potion, confirms count 1, verifies Crystal-style belt placement with total quantity preserved, archives two split screenshots, and records `inventorySplitFlow`.
- 2026-04-26: Stage 5 UI smoke now opens Delete Item for Blue Potion, confirms the drop, verifies quantity decreases and a ground-drop label appears, archives two item-drop screenshots, and records `inventoryDropFlow`.
- 2026-04-26: Character RemoveItem now sends the Crystal-shaped inventory-grid target with the first free bag slot, and Stage 5 UI smoke verifies Dagger leaves equipment and returns to bag1 slot 4, archives `stage5-character-remove-dagger.png`, and records `characterRemoveFlow`.
- 2026-04-26: Stage 5 UI smoke now clicks Red Potion directly in the belt, verifies quantity decreases before the existing hotkey path, archives `stage5-belt-mouse-use-red-potion.png`, and records `beltMouseUseFlow`.
- 2026-04-26: Stage 5 UI smoke now opens Sell Item for Dagger, confirms without an active sell service, verifies Dagger and gold are preserved, archives two sell screenshots, and records `inventorySellFlow`.
- 2026-04-26: Stage 5 UI smoke now opens Store Item for Dagger, selects a warehouse slot without an active storage service, verifies Dagger and existing storage contents are preserved, archives three store screenshots, and records `storageStoreFlow`.
- 2026-04-26: Stage 5 UI smoke now opens Take Back for stored Red Potion, selects an inventory slot without an active storage service, verifies inventory/storage quantities are preserved, archives three take-back screenshots, and records `storageTakeBackFlow`.
- 2026-04-26: Storage Protect is now reachable before a password exists, and Stage 5 UI smoke opens/closes Set Storage Password without submitting credentials, archives `stage5-storage-password-panel.png`, and records `storagePasswordFlow`.
- 2026-04-26: Stage 5 UI smoke now fills Set Storage Password, archives mismatch and no-service submit screenshots, verifies mismatched confirmation keeps submit disabled, and verifies matching submit without an active storage service leaves `hasStoragePassword=false` with no-service feedback.
- 2026-04-26: Stage 5 UI smoke now captures 71 screenshots and records Mail/Report/NPC panel state, broad Stage 5 systems state, guild/group chat filters, Character repair/special-repair, ground item/gold pickup, combat target state, system-menu QA and transfer-list routing, Battle Focus spell casting, and compact inventory panel bounds.
- 2026-04-26: Stage 5 UI smoke now captures 85 screenshots and records login/select lifecycle, confirmed character delete/recreate, compact inventory/storage/character/system-menu/chat-settings bounds, NPC dialog link-capable state, and the existing broad gameplay/system matrix. Map API and minimap asset smoke outputs are archived under `docs/generated`, and WS load refresh is 64/64 ready with 0 errors.
- 2026-04-26: Stage 5 UI smoke now captures 88 screenshots and records advanced Stage 5 systems state for trade item/cancel, shop gold purchase, auction buy/cancel, conquest end, hero behaviour, mining/craft, and mail delete state. Compact Mail and Report panel bounds are now asserted and archived as `stage5-compact-mail.png` and `stage5-compact-report.png`.

## Human-Only Acceptance Boundary

Automation can verify crashes, route completion, DOM state, screenshots, packet traces, and data snapshots.

Human acceptance is still required for:

- whether the screen visually feels like Crystal;
- whether mouse targeting and item interaction feel right;
- whether combat feedback, animation pacing, and panel layering are acceptable;
- whether small visual differences should be fixed or accepted.

## 2026-07-23 Deterministic Visual-Parity Closeout

- The current same-account gate uses overlay-free Crystal/Web `1024x768`
  captures at Bichon `0 @ 328,275` with explicit server light pairing.
- Dawn improved from r29 full/world changed pixels `36.4%/40.2%` to final r33
  `24.2%/26.1%`; world MAE fell from `18.845` to `11.987`.
- Final Night r32 remains at full/world `12.5%/12.6%`, matching the r26
  `12.4%/12.6%` baseline and proving the Dawn/Evening correction is scoped.
- HUD experience fill now follows Crystal's `(1004 - 3) * ratio` clipping,
  HP uses the native source crop, chat scrollbar rows match native geometry,
  minimap projection follows Crystal's reverse quantization, and AI 6 monsters
  retain the native green radar colour through observer re-seeding.
- The capture/login path now waits for `Connected`, retains one latest pending
  action, ignores stale-socket events, verifies cursor parking, and redacts
  serialized secrets. r33 records zero critical errors and zero 404s.
- Final strict headed captures pass 28/28 assertions on both selected WebGPU
  and forced WebGL2. Each sends and acknowledges four ordered moves, settles
  at `328,275`, and leaves no pending movement/map transaction or browser
  error. The dual-backend runtime smoke also remains fully green.
- The captured Bichon source closure adds 555 deterministic Crystal map PNGs.
  Release preflight sees 39,401 manifest assets, 12,015 original map PNGs,
  8,228 packed entity sprites, and 99.76% renderable map-frame coverage.
- Remaining visual gaps are GDI typography, deterministic chat content, and
  independently moving entity/animation phases. They are no longer classified
  as movement, camera, or map-transaction failures.

## 2026-07-23 Quest Marker GPU Occlusion Regression

- Live inspection confirmed that all four nearby Scarecrow entities resolve
  through `Monster/005`; the full pack contains 234 drawable frames and the
  separated one-tile capture shows the native thin straw/skeletal body. The
  reported invisibility was player overlap and low contrast, not a missing
  atlas or failed entity render.
- NPC quest markers were still mounted inside `viewport-sprite-overlay`.
  With the Bevy entity renderer active, the GPU canvas at z-index 2 covered
  that DOM layer even though the quest state and marker image were present.
- Quest markers now live in the independent `viewport-entity-overlay`, share
  the imperative entity-motion registry, retain Crystal sprite offsets and
  NPC activation, and carry the NPC object id for deterministic QA lookup.
- The live account currently has the available stage, so Crystal correctly
  shows yellow exclamation markers. Accepted/in-progress and ready-to-turn-in
  stages continue to select white and yellow question markers respectively.
- Evidence:
  `docs/generated/player-qa/r41-quest-marker-overlay/r41-quest-marker-overlay-final.jpg`
  shows three visible yellow exclamation markers above the WebGPU scene.
  `elementsFromPoint` reports the marker above the drop overlay and canvas.
- Verification passed:
  `node apps/web/scripts/test-player-frames.mjs`,
  `npm run typecheck --prefix apps/web`, and `git diff --check`.

## 2026-08-12 Quest Window Stage Anchor and Localization

- Original-client extra windows now portal into `.client-stage-frame`, so their
  1024x768 coordinates inherit the same stage translation and scaling instead
  of resolving against the browser viewport. This closes the quest-window
  top-edge drift visible on letterboxed desktop viewports and protects the
  other extra windows from the same coordinate-space bug.
- Quest-log filter/stage labels now use the generated dotted localization keys.
  Quest IDs 1, 2, 5, and 154 are localized at the presentation boundary,
  preserving canonical gateway quest packets while rendering Chinese titles,
  summaries, objectives, progress, tracker text, rewards, and structured
  objective labels. Quest 154 now renders `皇帝的难题` consistently across
  available, in-progress, ready-to-turn-in, and completed states instead of
  leaking the original `Emperors Problem` title into the Chinese quest log.
- Automated geometry passed at 2208x1812 and 1440x900. At 1440x900 the stage was
  `(208,66)-(1232,834)` and the quest window was `(770,71)-(1082,515)`, with
  `.client-stage-frame` as its direct parent and no browser errors. Local
  screenshots are under `output/quest-window-stage-anchor/`.
- Verification passed: `npm run test:responsive-stage`,
  `npm run test:quest-localization`, `npm run typecheck`, and
  `git diff --check`. This is Candidate automation evidence; authenticated
  human visual acceptance on the production-shaped game page remains required.

## 2026-08-12 Right Menu Quest Log Button

- The Crystal `Title/567` right-side menu already includes an unused paper
  slot at `(3,107)`. That slot is now the Quest Log action, preserving the
  original 36x282 menu geometry and every existing button position.
- Clicking the paper icon toggles the standalone Quest Log and closes the
  right-side menu. Reopening the menu exposes the pressed state; clicking the
  same icon again closes the Quest Log. The existing `Alt+Q` shortcut remains
  available.
- Automated browser acceptance covered closed -> open -> closed, verified that
  the quest window remains parented to `.client-stage-frame`, and recorded zero
  console or page errors. Evidence is under
  `output/system-menu-quest-button/quest-button-active.png` and
  `output/system-menu-quest-button/quest-open.png`.
- Verification passed: `npm run test:system-menu-quest-button`,
  `npm run test:responsive-stage`, `npm run test:quest-localization`,
  `npm run typecheck`, and `git diff --check`. This is Candidate evidence;
  authenticated human acceptance on the production-shaped game page remains
  required.
- Follow-up live-page acceptance exposed a stale portal-host regression after
  Fast Refresh: `showQuestLog` and the button pressed state were true, but the
  quest window was still being rendered into a detached pre-refresh stage node.
  The extra-window registry now re-resolves `.client-stage-frame` after each
  registry render and refuses disconnected hosts. In the existing authenticated
  game tab, the same button produced one visible quest window under the current
  stage, then closed it on the second click, with zero page errors. A forced
  stage-remount regression capture and text state are archived under
  `output/system-menu-quest-button/remount-regression/`.

## 2026-08-12 Entity flicker and monster death assets

- Root cause: the packed entity path could rebuild for each changing visible
  frame set, while actor URLs preferred an older R2 release and local metadata
  could remain cached at the previous 80-frame export. This combination caused
  intermittent missing sprites and hid later Attack/Struck/Die/Dead frames.
- The default is now a stable prebuilt entity atlas. Dynamic repacking requires
  explicit opt-in; Bevy can use partial prebuilt coverage with per-layer image
  fallback, while raw WebGL2 only hides DOM sprites when all sources are covered.
  Actor libraries retained in the web bundle prefer same-origin assets and local
  metadata uses `no-store`.
- Original Crystal exports and the atlas were rebuilt. Critical closure is:
  `000=128/3`, `003=232/8`, `004=232/8`, `005=234/7`, `007=448/7`,
  `010=164/8`, and `012=224/7` (frames/actions). `007` exposes Die and Dead via
  the live local metadata endpoint. The entity atlas contains 9,650 frames over
  seven pages.
- Forced-Bevy browser evidence:
  `/tmp/mir2-render-monster-final-20260812/report.json` (map `0`) and
  `/tmp/mir2-render-field-final-20260812/report.json` (map `1`) both pass with
  rendered floors, full visible-source coverage, a prebuilt hit, zero live
  atlas builds, zero asset gaps/404s, and zero console errors.
- Local Candidate is green. Production remains open until the rebuilt asset
  release is uploaded to R2 and the matching web build is deployed and observed
  during real combat.
- Upload-ready scatter-frame evidence is
  `docs/generated/remote-assets/20260812-monster-render-death/remote-asset-release.json`:
  40,944 files, 237,928,682 bytes, zero missing files, with object prefix
  `mir2/v/20260812-monster-render-death`.

## 2026-08-18 Quest material inventory icon closure

- Root cause: quest material records referenced valid original `Items.Lib`
  frame indices, but the curated web export and R2 release contained only 316
  item frames. The inventory metadata guard consequently substituted a
  transparent placeholder for absent frames, so the item existed while its
  picture appeared blank.
- The curated export now contains every frame referenced by generated quest
  carry-item and item-task data: 71 quest material records resolve to 60 unique
  item images. This adds 44 previously absent original frames and raises the
  checked-in `Items` export from 316 to 360 frames.
- `assets:remote:build` now asserts the complete quest-item icon set before
  staging a release, and the R2 release workflow runs the same regression test.
  A future quest material without both a PNG and matching `Items/meta.json`
  entry therefore fails CI instead of shipping a transparent icon.
- Inventory, storage, delete, sell, and split previews now fall back once to the
  original unknown-item icon when an image request fails, then hide the broken
  image to avoid an error loop. Opening the inventory also returns to the first
  bag tab, and the tab chrome remains above the item grid.
- Local Candidate evidence: quest icon closure reports 71/71 records and 60/60
  unique images, TypeScript compiles cleanly, asset URL tests pass, the local
  route for `Items/412.png` returns HTTP 200, and the production-shaped login
  page renders without a black/blank stage. The full item-lifecycle browser
  script was blocked before character selection by the local gateway account
  fixture and is not claimed as a passing gameplay certificate.

## 2026-09-09 native trade drag fix

The intermittent item drag is fixed by replaying ordered Winit/Bevy WindowEvent input and retaining a preselected source until release. Pointer tracking survives pending/modal gesture cancellation. Native UI 624/624 and Windows host 561/561 serial pass. Exact signed EXE B62330DFA03EB8A2A5976C0662A327D95A2E65A4721C0B3E2D0ECC3F9DE75599 passed seven native drag operations: deposit, occupied swap, preselected-source move, retrieve, redeposit and partial merge, with matching ACKs. This supersedes the prior drag-open finding only; full-game acceptance remains false. Evidence: [drag fix](generated/player-qa/native-trade-drag-fix-20260909/README.md).
