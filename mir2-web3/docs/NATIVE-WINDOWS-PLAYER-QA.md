# Windows 原生可玩闭环验收清单

## R23：已实际启动，分辨率与九语基础显示已检查；游戏内验收待登入（2026-10-10）

用户现已授权操作游戏。本机正常启动器首次更新因四个资源404失败并回落R19；按既有清单补齐772163B后，重试实际激活sequence18/R23，安装EXE及四项资源完整哈希匹配。签名、清单、feed和网关/Caddy进程保持，未发布本制作加工/赌石分支。逐文件检查仍耗时6分54秒，性能问题尚未解决。

实际切换1024×768自动→1280×960并恢复；登入前九语标签均已逐个观察，俄语/越南语长按钮在边界内换行。16张有效窗口截图已保存。客户端恢复繁中和自动模式，空登入页等待用户手动登入；没有自动执行账号认证，没有进入游戏世界，22项玩法仍not_run，不代表Candidate100或人类前端验收完成。旧构建记录和其他正文保留，各自证据范围不能合并。

完整记录：[本轮原生实机记录](NATIVE-PLAY-ACCEPTANCE-20261010.md)、[实际结果](generated/native-play-acceptance-20261010/acceptance-result.json)、[证据索引](generated/native-play-acceptance-20261010/evidence-index.json)。

## Source50：新完整 Web 包已构建，Sharp 入口兼容修复；实玩待验证（2026-10-09）

最新完整 Web 已更新为 Source49 Next＋Source50 Thin，包含当前共享 UI 与九类任务动作源码；本轮不新增界面操作。Source49 完整 Core/PUI/NPC 三包和 WebGPU、WebGL2、共享 WebGL2 三 renderer 均由原 builder 实际编译并通过 metadata/name/normalized ABI、原 WASM/JS/default-gzip 预算及不可变发布；静态 WASM 校验获本次“继续”授权，没有实例化游戏。Core 三次和 renderer 四次 Cargo 严格串行、原50GiB/2000ms/15000ms及PolicyB保持，均完成、退出并释放。

原 Next build --webpack 实际 exit0，162013ms，19556项完整输入在原调用前后匹配，严格 TypeScript 和13页静态生成通过。Thin 首次因当前 Sharp0.35.3将 sharp.node 导出指向 index.cjs 失败（实际 exit1/25289ms）；本轮仅修真实打包 helper，支持包根有界单句入口并检查其实际版本化 .node，保留版本、许可、DLL、regular file/path/COPY_EXCL和原预算。55个现有Sharp输入仍等于此前恢复记录，没有安装或混入旧版本。该 helper 只供 Thin 消费，Next 原编译区间及不变产物明确限定承接；不声称当前整个旧广义快照仍匹配。

Source50 原 Thin --skipBuild true 实际 exit0，38463ms，55303项完整当前输入在原调用前后匹配。新包7331 regular files、778目录（含根）、0链接、373987154B，原377487360B上限余3500206B；三套renderer和Core/PUI/NPC均与已编译版本一致。包位于 apps/web/.mir2-thin-client-web-windows-catchup-source50-20261009-01。231项JSON压缩及59条原NFT追踪警告保留，产物与依赖另有只读复核；原失败输出、历史旧08及不可变旧版本均保留。

继续遵守“继续代码，暂不操作界面”：真实资源初始化、登录→任务/战斗→保存退出重登、移动真机及人类前端验收仍未执行；省略媒体的不可变资源origin和覆盖仍待真实验证，构建成功不证明在线可玩。Matrix11仍103 shared＋206 legacy＋8 common，309/317≈97.5%仅有界代码记录，overallPercentage=null、Candidate100=false、goal未完成，无可信可玩日期。用户主4932B tsconfig保留；外来Cargo/simulation/后端及制作加工文档正文保留且排除本轮提交。

证据：[本轮交付](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-source50-build-delivery01.json)、[64份原始记录](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-source50-build-raw-evidence02.json)、[完整Thin实际结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-source50-thin-full-actual01.json)、[实际产物复核](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-source50-thin-full-actual-independent-review01.json)、[依赖及警告复核](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-source50-thin-dependency-actual-review01.json)、[Source47实际推送](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-source47-publication01.json)。本轮提交推送随后另存真实记录，下方原正文及其他工作保持。

## Source47：构建配置副本入口通过严格类型检查；完整 Web 包待生成（2026-10-09）

next.config.ts 新增可选 MIR2_NEXT_TSCONFIG_PATH，未设置时保留原行为，未放宽类型检查。原严格 TSC 实际 exit0、零诊断、12188ms，17136声明输入在调用前后匹配；用户主 tsconfig 原4932B保留。路径校验为代码与源码审阅结果，尚未实际执行 Next 配置。

完整 Core/PUI/NPC 859项与 Bevy 607项声明输入已核对并绑定原 builder；旧36套renderer版本及12份公开flat文件字节保留。未来 Thin 的 report 参数只修正等价路径分隔符，未启动构建。原静态 WASM API边界问答未收到回答，完整runtime校验、不可变发布和新Next/Thin仍未执行；最新完整Web仍Source25。真实登录/任务/战斗/保存/重登及移动验收未运行，overallPercentage=null、Candidate100=false，goal active，无可信可玩日期。

证据：[实际结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-source47-finite-result02.json)、[20份原始文件](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-source47-raw-evidence02.json)、[严格TSC](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-source47-tsc-actual01.json)、[限定实际审阅](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-source47-finite-actual-limited-review01.json)、[Source46实际推送](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-source46-publication01.json)。本轮提交推送随后另存实际记录，下方原正文及其他工作保留。

## Source46：更新 EXE 实际编译通过；完整 Web 打包边界待确认（2026-10-08）

原守卫下最新 Native EXE 实际编译 exit0，835593ms，保留原 dev/debug（optimized＋debuginfo）配置。917项隔离声明输入在实际调用前后重核匹配，原50GiB/2000ms/15000ms和PolicyB保持；实际C197697568768B、freshness保守上界74ms，完整completed/exited/disposed后才接受。新EXE106106368B已保存独立副本，旧Source31 EXE106137600B同字节历史副本保留。正常Native编译闭包成立；5个仅cfg(test)未声明资产不计全测试。本轮未新跑Rust/Web/TSC、未启动EXE、未打包完整运行资源。

三套renderer编译与静态转换继续以Source45限定结果为依据，完整metadata/name/normalizedABI/defaultgzip/init未通过。原完整builder可条件性用现有CargoGuard直接逐次守卫（Bevy通常4nonce、Core3nonce），没有fresh预构建候选直接接纳入口；原Bevy锁若存在即拒绝启动，不读取锁内容或查询PID。当前任务保留禁WASM API约束，已问一次是否允许原静态构建校验，未收到回答前不执行依赖步骤；旧immutable版本与Next08/Thin08保护。最新完整Web仍Source25；登录/任务/战斗/保存/重登、移动及人类验收未执行，overallPercentage=null、Candidate100=false、goal active，无可信可玩日期。

证据：[本轮有限结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-source46-finite-result01.json)、[实际Native编译](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-source46-native-actual01.json)、[14份原始文件](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-source46-native-raw-evidence01.json)、[原完整流程守卫路线](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-source46-original-builder-guard-route-review01.json)、[Source45实际推送](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-source45-publication01.json)。Source45远端429fe0eb2a860da30f6327702b29395df62c3afc已核验，本轮随后单独保存实际提交推送；用户tsconfig及外来后端/文档正文保留并排除提交。下方历史保留。

## Source45：最新 renderer 编译与动作回归通过；完整包及实玩仍待完成（2026-10-08）

默认共享入口/任务四原脚本实际48/48 TAP通过；Hero原Stage5脚本实际1/1文件TAP、501逻辑组通过，其中原494组字节和断言保留，新增7组/11正向场景覆盖装备、卸装、合并、跨背包转移及补药配置。实际SharedHost→Page发送/claim→唯一台账→回执/完整快照接收路径执行，最终UID/数量/配置另有断言；Prepared/Ready、socket和权威结果仍为有限夹具，非真实服务器或玩家验收。两个输入区间分开记录，不声称整个当前checkout通过。

三个renderer实际release Cargo及六次原bindgen/Binaryen静态转换全部exit0；优化WASM为31467818/17745325/32429102B，均在原32505856B内，共享WebGL2余76754B；JS均在原204800B内。917隔离Rust输入与当前17136项Web声明输入最终重核匹配，三Cargo严格串行、实际PolicyB关闭，原50GiB/2000ms门槛保留。本轮未新跑Rust测试、Native测试或TSC；源未变的Source44类型检查仅限定承接。

Core/PUI/NPC只按56个相关源文件＋4个控制文件未变及18份产物pin限定保留Source33静态候选，旧完整workspace不承接。Windows EXE需要fresh编译；完整renderer metadata/name/ABI/defaultgzip/init、不可变发布与Next/Thin打包仍待完成。最新完整Web仍Source25，尚不含Source26–45。功能记录309/317≈97.5%含旧实现；共享分类103/317≈32.5%也不是总体进度。overallPercentage=null、Candidate100=false、goal active，可玩日期未知；继续代码且暂不操作界面，登录/任务/战斗/保存/重登与移动、人类验收未执行。

证据：[本轮实际结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-source45-finite-result01.json)、[74份原始记录](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-source45-raw-evidence01.json)、[最小交付输入审阅](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-source45-delivery-input-review01.json)、[Source44实际推送](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/web-hero-source44-publication01.json)。Source44远端ad6f915a693e1322b69afeed926f36318caefdb4已核验；本轮随后单独记录实际提交推送。用户tsconfig及外来后端/文档正文保留并排除本轮提交，下方历史保留。

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


> 2026-09-03 relocated-launch map repair: the native map locator resolves a
> junction's physical asset root before looking for the development map pack.
> Startup now requires/decodes map 0 and no longer labels an index alone as
> Full ready. The reported Bichon (302,634) viewport resolves 849 map draws and
> 221 local image references; Windows host tests pass 537/537 serially from a
> non-repository directory using the relocated asset entry. Three default-
> parallel GameShop queue failures are separately recorded, not claimed fixed.
> The user explicitly resumed Computer Use; the repaired EXE was opened at
> login. Manual login/in-game visual verification remains pending. No assets,
> store or Gateway were changed. Evidence:
> `docs/generated/player-qa/native-ui-parity-20260903-map-relocation/README.md`.
> All 33 IDs and global visual/feel acceptance gates remain open.

> 2026-09-03 trade completion source/headless checkpoint: shared preparation
> no longer closes the source-correct trade windows by emitting completion.
> Successful/saved delivery completes once, including an empty incoming side;
> pending/rejected/unknown outcomes do not. Simulation 1491/1491 + dedicated
> 7/7, Gateway 672 passed / one existing ignored and all 1380 native/client tests pass. Evidence:
> `docs/generated/player-qa/native-ui-parity-20260903-trade-completion/README.md`.
> This supersedes only premature completion. Original invitation/private
> routing, positive gold additions/immediate escrow, unlock/re-edit, capacity
> failure retaining offers, cancellation/mail and item operations remain.
> Native cells are read-only. No GUI/input/screenshot occurred while Computer
> Use remains paused after Escape. After explicit resumption, verify populated
> final-source windows and real paired transactions against same-state Crystal.
> All 33 IDs, package/light/DPI/soak/legal/signing/human gates remain open;
> visualAccepted=false, accepted=false, globalParityPercent=null; PR #250 Draft.

> Historical 2026-09-03 accepted-exchange TradeDialog checkpoint: source
> own/guest 204x152 windows and original controls, ten fixed sparse cells each,
> independent inventory, exact current-count icons/hints and basic amount/
> lock/close state are covered. Explicit exchange/unlock ownership survives
> coalesced social events; own-offer projection is identity-bound/read-only.
> Native UI 591/591, Windows 534/534, runtime 212/212, UI core 43/43 and
> item-icon 11/11 with 924 images pass. The asset verifier matches 561 PNGs,
> preserves all 552 prior Prguse/Title PNGs and rejects 32 negative controls.
> Report: `docs/generated/player-qa/native-ui-parity-20260903-trade-dialog/README.md`.
>
> Do not sign off a complete trade route: grids are read-only, source item
> operations and invitation/cancel messages remain open, and Candidate's
> unilateral S.TradeConfirm plus absolute/deferred-debit TradeGold differ
> from Crystal. Fix and verify transaction phases/conservation before the
> full route. No backend code changed or full-server suite ran this round.
> Computer Use remains paused after user Escape: no GUI launch, input or
> screenshot occurred. After explicit resumption, bind the final-source EXE
> and real state to populated own/guest windows, invitations, gold additions,
> item operations, lock/unlock, cancel and mutual settlement, with same-state
> original pairs. Full editing/GDI, Gold 106.wav, overlap/topmost, package/
> light, actual DPI, soak, legal/signing and human gates stay open. Preserve
> all 33 IDs; `visualAccepted=false`, `accepted=false`, `globalParityPercent=null`.

> 2026-09-03 primary-item source/logic checkpoint: bag, belt, equipment,
> personal storage and NPC goods use original alpha-bound GetTrueSize while
> drawing the complete bitmap. Legitimate source Items/0, stackable count 1,
> source 36x32 equipment/storage cells, oversized draws and belt late-load/
> change/clear are covered. Warehouse-side/trade/amount icon regions share
> the helper; their whole-window and item-operation gaps are not closed.
> All 1003 exported original PNGs match RGBA/metadata and 5015 exact native
> node geometries pass. Final native UI 562/562, Windows 528/528, runtime
> 212/212, UI core 43/43 and item-icon 11/11 plus 924-image coverage pass.
> Evidence: `docs/generated/player-qa/native-ui-parity-20260903-item-true-size/README.md`.
>
> Computer Use is still paused after user Escape; no application window,
> input injection or new screenshot occurred. After explicit resumption,
> bind the final binary/state to populated primary/NPC/trade views, real
> quantity changes, zero/oversized images and original same-state pairs.
> This source/node evidence is not a GPU pixel or same-EXE acceptance claim.
> Preserve every original-pair/package/light/DPI/soak/human gate and all 33
> IDs. `visualAccepted=false`, `accepted=false`, `globalParityPercent=null`.
> The historical Guild note's primary image-layout/zero gap is superseded;
> source operations, state overlays and remaining specialized layouts are not.

> Historical 2026-09-03 native Guild storage source/logic checkpoint: the original
> 8x14 grid, 8-row viewport, stable slot IDs, source integer scrolling,
> alpha-bound current-count icons, valid Items/0, original tab/scroll art and
> gold MirAmountBox now have headless coverage. Final native UI 551/551,
> focused Guild 44/44, Windows 527/527, runtime 212/212, integration 4/4,
> UI core 43/43, protocol 40/40, game-data 39/39, all 924 item
> images and 41 direct original RGBA/geometry checks pass. Current host and
> integration outcomes are recorded separately in
> `docs/generated/player-qa/native-ui-parity-20260903-guild-storage/README.md`.
> Source GetTrueSize differs from PNG dimensions for 550/1003 exported Items,
> with 478 different 35-pixel-cell offsets. This round corrects Guild/coin
> only. Primary item and NPC centring, including older claims based on
> PNG-frame pixel checks, is still an explicit open leaf.
>
> No fresh EXE launch, screenshot or foreground input was performed: Computer
> Use remains paused after user Escape. Once explicitly resumed, the manual
> route must bind the final binary and authoritative fixture to populated
> first/last storage rows, real wheel/drag, deposit/withdraw/zero/cancel,
> identity/rank/balance changes and matching packets; never mutate ordinary
> player state merely for a screenshot. Guild item operations, state overlays,
> full text/clipboard/GDI behavior, window overlap/movement and shared throttle
> remain open, as do original paired-state, trusted package/light, actual DPI
> and human acceptance. `visualAccepted=false`, `accepted=false`,
> `globalParityPercent=null`; all 33 prior backlog IDs remain.

> 2026-09-03 native Crystal NPC goods-cell checkpoint: the fresh Windows EXE
> SHA-256
> `159B13E722451C6F44B036C6B3ABD141E19362EDB28ED29180F34C6849A7DD8A`
> followed the real login -> Scott -> View route and rendered populated NPC
> goods at BichonProvince `(288,616)`. Baseline capture
> `docs/generated/player-qa/native-ui-parity-20260903-npc-shop/mir2-in-game-1788375662467-1.png`
> has SHA-256
> `B62E187B4042434875FACF9130F20A0D443CF72C4D63462A1E8C084FBDC1FF6C`;
> selected/hover capture
> `docs/generated/player-qa/native-ui-parity-20260903-npc-shop/mir2-in-game-1788375687703-2.png`
> has SHA-256
> `DCECFE0FC836F6CBA4961D40E6F259622DAAA075059CFF13817F8C9B304349DA`.
> Both sidecars bind run `npc-shop-20260903-r2`, `panel=NpcShop`, 1024x768,
> DPI 1.0 and the same world state. The second visibly freezes Crystal's
> `205x32` Lime-selected cell, true-size centred icon, separate name/count/
> price labels and shared item hint. `NPCGoods.HideAddedStats` is preserved
> through the packet/model/runtime chain and has a regression proving it
> hides additions and `Cursed`, not base/bind text. Source authority is
> `MirGoodsCell.cs:20-140`, `NPCDialogs.cs:1071-1082,1348`,
> `ServerPackets.cs:3082-3104` and `GameScene.cs:4199`. Native UI passes
> 514/514; runtime 212/212; Windows 519/520, with only the pre-existing Archer
> atlas fixture failure. Full report:
> `docs/generated/player-qa/native-ui-parity-20260903-npc-shop/README.md`.
> Sidecars remain `eligible=false`; duplicate/sub-goods topology, the other
> specialized item-surface layouts/captures, trusted build/light provenance,
> 100/125/150% DPI and human Crystal comparison remain open.
> `visualAccepted=false`, `accepted=false`, `globalParityPercent=null`.
>
> 2026-09-03 native remaining item-tooltip surface checkpoint: packet adapters
> now retain complete tooltip source for NPC shop, GameShop, fixed/selectable
> quest rewards, guild storage and trade. Automated coverage verifies Crystal's
> actual-instance versus synthetic-preview distinction, GameShop durability and
> count, Quest's separate reward quantity, viewer-specific class/level data,
> recursive socket/bind fields, ambiguous-index rejection and the common hover
> lifecycle. Native UI passes 511/511. Windows passes 519/520; the sole failure
> remains the previously recorded Archer atlas fixture expecting
> `/ARArmour/00/24.png`, while a current Debug executable builds successfully.
> No populated same-EXE screenshot for these five surfaces has been admitted,
> and exact panel geometry/hit regions, NPC hide-added-stat behavior, DPI and
> human Crystal comparison remain required. `visualAccepted=false`,
> `accepted=false`, `globalParityPercent=null`.
>
> 2026-09-03 native Crystal item-tooltip checkpoint: an isolated latest
> Gateway created and persisted the fresh `TooltipR1` fixture, then the exact
> Windows EXE SHA-256
> `5257E859B4AB173A8076B58778C59D09D291A7EB90F0FCFA38F696E46181A56F`
> entered BichonProvince `(288,616)` and opened Bag1. Real Windows pointer input
> selected slot 0; Escape closed the item-operation layer while leaving hover
> active, and F12 captured the renderer-owned WoodenSword hint at
> `docs/generated/player-qa/native-ui-parity-20260903-item-tooltip/item-tooltip-in-game-1788370099170-2.png`
> (SHA-256
> `82F2D5ACAB20874FB31D3C3B3EF8EA105D03495BD32ACED27D7BDE0EE6B78AB`).
> The visible order covers name/quality, type, weight/durability, DC,
> requirements and price. Adjacent sidecars freeze the same run id, panel/page,
> dialog origin and world state; `process-provenance.json` freezes process paths
> and hashes. Native-ui passes 509/509; Windows full suite is 517/518 with only
> the pre-existing Archer `/ARArmour/00/24.png` fixture failure. Sparse shop,
> reward, guild-storage and trade models, trusted build/light provenance, DPI
> and human Crystal comparison remain open. `visualAccepted=false`,
> `accepted=false`, `globalParityPercent=null`. Full report:
> `docs/generated/player-qa/native-ui-parity-20260903-item-tooltip/README.md`.

> 2026-09-03 native Inventory movable checkpoint: the final current-tree r5
> EXE (SHA-256
> `F08FC69F744BC9D6895A7756CF98AAF5A69EEEF4CA8AF10F65BA78D8663B33D3`)
> used real Windows pointer input to move the source-sized `316x236`
> InventoryDialog from `(0,0)` to `(275,208)`. Both captures share run id
> `inventory-drag-20260903-r5`, character, Bag1 contents and BichonProvince
> `(287,616)`:
> `docs/generated/player-qa/native-ui-parity-20260903-inventory-drag-final/inventory-drag-inventory-1788365024393-1.png`
> (SHA-256
> `64F8A6A4ECB500F40C508ED804FDA686DEEF11A4FDAF507601680202064A5B5C`)
> and
> `docs/generated/player-qa/native-ui-parity-20260903-inventory-drag-final/inventory-drag-in-game-1788365096792-2.png`
> (SHA-256
> `AED8F77D8F8F6A43E80F985A13748ECFD11BA26C33B462A406F01F6185C8CFE3`).
> The sidecars independently record the two exact `inventoryLocation` values.
> Source authority is `InventoryDialog.cs:25-31` plus generic drag/clamp/
> release behavior at `MirControl.cs:852-935`; original `Title/196.png` is
> `316x236`. Drag regressions pass 4/4 and full native-ui passes 496/496.
> Trusted build/light provenance and human visual acceptance remain open.
> `visualAccepted=false`, `accepted=false`,
> `globalParityPercent=null`.

> 2026-09-02 native Inventory/DeleteMode checkpoint:
> `MIR2_NATIVE_CAPTURE_AUTO_SCREEN=inventory-delete` now waits for an
> authoritative in-game inventory, opens Bag1, enables the real UI DeleteMode
> and selects the first eligible ordinary bag item. A disposable character
> with four server-backed items exercised this path through the local Gateway;
> the renderer capture is
> `docs/generated/player-qa/native-ui-parity-20260902-inventory-delete/inventory-delete-final-inventory-delete-1788362950757-1.png`
> (SHA-256
> `50F30B959365E5E2F4B2F6C1BA677118A683A4A7E91E47FEB4E5E6953F8761B1`).
> It records the exact centred single-item Yes/No prompt and leaves YES
> untouched, so the fixture remains intact. The companion Quest-page icon and
> footer evidence is
> `docs/generated/player-qa/native-ui-parity-20260902-inventory/inventory-final-inventory-1788360618712-1.png`.
> The live stacked-item amount-box capture, trusted build provenance,
> authoritative light and human visual comparison remain gates.
> `visualAccepted=false`, `accepted=false`,
> `globalParityPercent=null`.

> 2026-09-02 native Run lighting-camera edge follow-up: the remaining straight
> dark strip was not a missing map binding and was not fixed by the shader
> guard alone. `follow_player` advanced the display-Hz `MainCamera`, while the
> offscreen `MirLightingBufferCamera` and main-pass `MirLightingComposite`
> stayed at the map-frame origin; movement therefore exposed the material's
> defensive `border_darkness` region as a ruler-straight band. The ordered
> presentation chain now runs `follow_lighting_camera` immediately after
> `follow_player` and copies the committed main-camera X/Y to both lighting
> followers while preserving their render-order Z and composite scale. The
> regression `lighting_buffer_and_composite_follow_the_presented_main_camera`
> locks a non-origin camera pose plus the original `1312x960` guarded composite
> size. Shared runtime passes 211/211. The full Windows suite reaches 511/512;
> its sole failure is the pre-existing unrelated Archer atlas fixture assertion
> for `/ARArmour/00/24.png`. A current-tree Debug EXE (84,956,160 bytes,
> SHA-256 `E1907E674386F6CEC7116B3BECA4C1E85D390CFFC9179C741F670B4DB4C32510`)
> was connected to the local `7010` Gateway and exercised three visible
> multi-cell runs (left, reverse diagonal and downward diagonal) in a 1024x768
> Bichon scene. All four viewport edges remained continuous; the soft lower-left
> night-light falloff remained scene-relative and did not become a fixed-width
> strip. This closes the diagnosed Run edge exposure, not final Crystal 1:1
> human acceptance. `visualAccepted=false`, `accepted=false`,
> `globalParityPercent=null`.

> 2026-09-02 native Gateway connection-loss follow-up: the user-visible
> `WebSocket protocol error: Connection reset` was a Gateway failure, not a
> renderer or local-motion regression. PID/port checks showed the old Gateway
> still listening on `7011/7111`, but `/health` accepted TCP and returned an
> empty response. Its runtime log contained repeated spectator append failures
> with Windows `os error 112` (`磁盘空间不足`), and E: had `0 MB` free.
> `.mir2-data/spectator` had grown to 22 generated JSONL recordings totaling
> 5,501,000,469 bytes (individual hourly files reached approximately 426 MB).
> Those files were moved recoverably to the host temp drive and the local
> developer launcher now sets `MIR2_SPECTATOR_RECORDING_ENABLED=0`, restoring
> the caller's prior value on exit. The launcher cleanup self-test passes.
> After restart, two health checks ten seconds apart both passed,
> `recordingEnabled=false`, `recordingErrorsTotal=0`, `persistedFramesTotal=0`,
> the spectator directory remained empty with zero E: free-space delta, and
> the native client retained one established WebSocket before returning to the
> Bichon game scene.

> 2026-09-02 native movement camera/thread-boundary follow-up: 60 fps Desktop
> Duplication evidence proved that the remaining "scene cannot keep up" defect
> was not network acknowledgement latency. Before the repair, one Walk/Run
> action exposed only whole-cell camera commits (`+48,-32` and `+48,0`), with
> no six intermediate Crystal phases. Native input was publishing the local
> command queue and self-camera window through Rust `thread_local!` storage,
> while Bevy could consume them on another worker thread. The render side
> therefore usually saw an empty bridge and only caught a command when the
> scheduler happened to reuse the producer thread. Native now uses bounded
> process-wide `Mutex`/`AtomicU64` bridges for those values; WASM keeps the
> original single-thread `thread_local!` path. Command-time local presentation
> is enabled for the complete Crystal movement window. The post-fix capture
> `.mir2-player-qa/video-20260901/native-ddagrab-fixed-v10.mp4` records one
> automatic route as six `(+8,-4/-6)` diagonal Walk phases, six `(+16,0)` Run
> phases and six `(+8,0)` final Walk phases, with no reverse flash or whole-cell
> snap. Shared runtime passes 209/209, native cross-thread bridge regressions
> pass, and Windows input tests pass 42/42. The final diagnostic-free Debug EXE
> is 84,921,856 bytes, SHA-256
> `B1EEED541E3E956202841988052FB4F8505FE66EFFA32620F0A3DA2F58C73BA7`.
> This is bounded local Debug/video evidence; the restarted native and Crystal
> windows remain the user's human feel gate. `visualAccepted=false`,
> `accepted=false`, `globalParityPercent=null`.

> 2026-09-01 Bichon Run edge-exposure/performance follow-up: the user-captured
> bright strip at the native viewport edge was not a missing map tile or a Zone
> rollback. The map renderer reported `missingBindings=0`; the defect was the
> world-space darkness/light composite being exactly stage-sized (`1024x768`)
> while the Crystal Run camera presentation temporarily translates by as much
> as 120 px horizontally and 80 px vertically. The first repair physically
> enlarged the offscreen light target to `1312x960`; after the user reported
> that movement felt unable to keep up, that approximately 60% extra offscreen
> fill was removed. The light texture is again `1024x768`. A `1312x960`
> composite mesh now supplies a shader-only `144x96` virtual guard: its central
> region maps 1:1 to the stage texture and out-of-range UVs emit the exact
> ambient darkness, so Run cannot reveal unmultiplied scene pixels without
> enlarging the rendered light target or making local lights screen-relative.
> Runtime tests pass 207/207 and the locked Windows Debug build passes. A fresh
> live four-direction retest exercised five Run plus eleven Walk commands; all
> 16 authoritative locations were confirmed, with zero correction/rollback,
> `missingBindings=0` throughout, and acknowledgement latency 13-77 ms
> (31 ms average). The guarded darkness remained present at the viewport edges.
> The running 84,921,344 byte Debug EXE is SHA-256
> `AEF8C39FC0CAEA18644045B6C0C661CCDCB836E9B9EE3E6170F502C729C535AE`.
> This is bounded local Debug evidence; final human movement/visual acceptance
> remains open. `visualAccepted=false`, `accepted=false`,
> `globalParityPercent=null`.

> 2026-09-01 Bichon building-opacity follow-up: the user's same-coordinate
> `(294,616)` native/Crystal comparison exposed a separate object-compositing
> defect after the global light cycle was aligned. Ordinary Crystal map PNGs
> already contain authoritative `.Lib` RGBA with binary alpha, but the native
> pack rebuilt every normal `Objects*` frame through the legacy black-key
> feather pass. That converted real edge-connected dark roof/wall pixels into
> partial alpha and let the pale ground show through. Local normal and additive
> frames are now staged byte-for-byte; map metadata alone selects normal versus
> additive runtime blending. The rebuilt map-0 manifest is SHA-256
> `E82E5573E98BDFC65B7EF463C9F09585F12805399E0783F7837D95F2D0AC1B1D`.
> Representative visible frames `Objects#7680/#7684/#7688/#7692/#7695/#7701`,
> `#8403/#8413` and `#8678` are byte-identical to their Crystal-export source,
> and the native pack regression now locks ordinary RGBA passthrough. A fresh
> live native restart and read-only 1024x768 observation at `(294,616)` showed
> opaque dark stalls, Blacksmith building and lower-right house matching the
> Crystal scene depth instead of the earlier washed-through buildings. This is
> bounded local Debug evidence; final human acceptance remains open.
> `visualAccepted=false`, `accepted=false`, `globalParityPercent=null`.

> 2026-09-01 Bichon map/lighting/movement retest: the native map reader now
> keeps Crystal's fixed `16x17` cell scan and extends Middle/Front object
> lookahead by 25 bottom rows, so bottom-anchored tall buildings are no longer
> cropped out. Startup also requires `generated/native-map-keyed/manifest.json`
> instead of accepting a partial asset root. The regenerated map-0 pack is
> SHA-256 `E82E5573E98BDFC65B7EF463C9F09585F12805399E0783F7837D95F2D0AC1B1D`
> with 7,672 refs, 4,703 entries, 4,520 keyed frames, 183 additive frames and
> the current all-animation-phase `missingSourceCount=2969`; the earlier 2,508
> value remains a historical pre-expansion baseline. `Objects#2723..2732` are
> present as one complete 10-frame family. Live Bichon diagnostics report
> `tiles=1090`, `standalone=334`, `missingBindings=0` and
> `incompleteFamilies=0`. The color mismatch was traced to native Debug forcing
> Day while Crystal was using its UTC light cycle; Debug now follows Crystal's
> dynamic UTC setting unless an explicit fixed-light override is supplied. No
> global gamma/darkening adjustment was added. Keyboard and mouse now share the
> same NewMove, collision, prediction and authoritative-ACK controller;
> right-click pathing continues after button release, ordinary Run predicts two
> cells and mounted/SwiftFeet Run predicts three, while authoritative Walk/Run
> cadence is 600 ms. Focused Windows input/map/presentation suites pass
> 42/42, 35/35 and 28/28, the native keyed-pack Node regression passes, and
> shared Zone passes 204/204. This is local Debug/automated evidence only; the
> running client still requires the user's human map-depth, building-animation
> and movement-feel acceptance. `visualAccepted=false`, `accepted=false`,
> `globalParityPercent=null`.

> 2026-09-01 character-select Last Online/preview stability retest: the user's
> side-by-side screenshot exposed two real regressions in the native Candidate:
> the selected character showed `Never` instead of Crystal's last-online value,
> and its animated body repeatedly flashed blank. The repaired local Debug path
> now shows `Scout` as `2026/09/01 12:42:48` using Crystal's UTC
> `LastLogoutDate`, exact Last Online row bounds, Arial 8 pt text, source
> `VerticalCenter` alignment and `yyyy/MM/dd HH:mm:ss` format. Preview layers
> preload and retain their complete
> 16-frame sets, share one animation clock, and keep the current images until
> every target layer is dependency-loaded. Fresh 1024x768 observations stayed
> nonblank across multiple 250 ms ticks and a full animation loop while the
> original Crystal client was independently returned to its select screen for
> comparison. Select tests pass 7/7; focused Simulation LastAccess tests pass
> 2/2, Gateway LastAccess tests pass 2/2, recovery replay passes 1/1, native
> protocol and Windows adapter regressions pass 1/1 each, and locked native and
> Gateway builds pass. This is bounded local implementation/QA evidence; the
> currently running Debug window is not a packaged exact-head Candidate and the
> user's retest remains the acceptance gate. `visualAccepted=false`,
> `accepted=false`, `globalParityPercent=null`.

> 2026-08-29 VIS-01 direct-frame/run/chat follow-up: the exact `02bb678747...`
> Candidate failed the user's visual retest because whole-actor flicker still
> occurred. Revision `a3121ce487c93ff37f2ca94d7d60d8e12bf9e5ea`
> therefore changes ordinary animation to update `Sprite.rect` directly on a
> retained full-page image, while retaining all observed atlas pages across
> page switches. It also fixes the missing `.png` suffix that made the chat
> frame appear transparent and maps right-click on empty world space to a
> Zone-authoritative Run intent. Shift plus a newly pressed direction remains
> the keyboard Run path. Dedicated regressions plus shared runtime 197/197,
> native-UI 430/430 and Windows 448/448 pass. Clean Candidate
> `WN-CANDIDATE-VIS01-DIRECT-RECT-RUN-CHAT-20260829` passes final nonvisual
> verification; its 67,430,912-byte EXE is SHA-256
> `4EB134ABDA3CC4981A4268CF4501E2ABB5BEDCD3E1C0F2E23F653008C7F8D57A`.
> It is running as PID 243288 against local Gateway PID 237188 (`/health` 200)
> for user inspection. Do not mark this visual pass until idle, walk, Run and
> chat-frame behavior are observed. Complete player actions, mouse combat and
> click-to-path, UI/chat, alternate classes, skills/VFX, map/monster semantic
> denominators, authenticated same-EXE WSS, real DPI, native 30-minute soak,
> formal publisher signing and human visual/audio/feel remain open.
> `visualAccepted=false`, `accepted=false`, `globalParityPercent=null`.

> 2026-08-28 VIS-01 exact-head motion Candidate evidence: clean revision
> `94f8e4f032643fdb826d6ef0ae82b360b4dcc83d` produced attested Candidate
> `WN-CANDIDATE-VIS01-MOTION-20260828`. The 67,435,520-byte Release EXE has
> SHA-256 `E40C5216A29DE870DA7898F0ACABE331E7310C583D249C2F66DC3210692050F4`;
> all 32,594 Candidate files are covered by the package identity chain and the
> payload aggregate is
> `167EB82528CD5ADEDA5621B170233FA2B8314540F11A95E65EB812ECA8D5B726`.
> Detached-CMS and final nonvisual verification pass. This closes exact-head
> build/package evidence for the two motion leaves, not visible play. The EXE
> was not launched, the internal certificate is not publisher Authenticode,
> and the keyed-map manifest still records 2,508 missing source entries.
> Authenticated same-EXE WSS, live mounted input, UI/chat, mouse combat,
> skills/VFX, complete monster/map denominators, 100/125/150% DPI, native
> 30-minute soak and human visual/audio/feel acceptance remain open;
> `visualAccepted=false`, `accepted=false`, `globalParityPercent=null`.

> 2026-08-28 VIS-01 mounted motion cadence/packet-clock checkpoint: revision
> `eb174e94eecde4a6e24f63d16616e2dfb9a03589` drives the native motion
> window from the current player movement descriptor instead of a global six-
> frame constant. Mounted Walk is now 8 x 100ms with matching right-direction
> sprite phases and camera cancellation; mounted Run stays 6 x 100ms. A live
> authoritative packet time window is consumed at its already-elapsed phase,
> while missing, future, expired, inverted or overflowing metadata safely
> falls back to the descriptor duration. Explicit movement frame counts are
> bounded to the Web-compatible 1..8 range. Focused 5/5 and full Windows
> 446/446 pass, and independent review is P0=0/P1=0. No exact-revision EXE,
> package, live mounted input route, screenshot or human-feel evidence exists
> for this head. The wider player/action/UI/VFX/monster denominators and final
> authenticated same-EXE WSS, DPI, soak, human and signing gates remain open;
> `visualAccepted=false`, `accepted=false`, `globalParityPercent=null`.

> 2026-08-28 VIS-01 continuous player-locomotion checkpoint: revision
> `532ddc6be0a0c38313fdd39fe9e0af82b883371b` makes overlapping native
> player Walk/Run segments continue from the prior segment's current
> fractional coordinate, restarts the visible phase with the matching
> authoritative sequence, and rejects the bounded self stale-source echo that
> previously could move both actor and camera back to the old tile. Only stale
> locomotion is replaced; queued combat remains FIFO, and monster/NPC paths do
> not use the player-only frame override. Runtime passes 194/194 and Windows
> passes 443/443, with independent P0=0/P1=0 player and Web/Crystal reviews.
> This head has not produced or launched an exact-revision EXE and has no new
> screenshot or human feel evidence. Mounted Walk eight-phase cadence,
> packet-carried timing, complete player actions, UI/chat, mouse combat,
> skills/VFX and final authenticated same-EXE live WSS, real DPI, 30-minute
> native soak, human acceptance and formal publisher signing remain open.
> `visualAccepted=false`, `accepted=false`, `globalParityPercent=null`.

> 2026-08-28 VIS-01 player-sprite geometry/package checkpoint: native Windows
> no longer invents a 48x64 rectangle when a player frame is absent from the
> entity atlas. The ten packaged player families use exact per-frame Crystal
> metadata as a verified fallback, including original offsets and transparent
> pixel hit/highlight behavior; other atlas misses still produce no layer.
> Source, staging and final-package closure all pass at 10 families / 34
> libraries / 22,944 declared frames. Full Windows tests pass 441/441, both
> Candidate self-tests pass, and exact revision
> `ef25aec83b8023003ae648b4a2955a4e9ec76362` produced nonvisually verified
> Candidate `WN-CANDIDATE-VIS01-PLAYER-GEOMETRY-20260828`. Its Release EXE is
> 67,398,144 bytes, SHA-256
> `1550B512930C54BA5356100B63976919A146E904F9A397D4EDE4CF653200FC3A`.
> The package carries a detached CMS statement verified with the internal
> certificate, while the EXE itself is not publisher-signed. At the
> 2026-08-28 observation, PID 225736 was a current-source debug/loopback
> inspection window, not exact-Release same-EXE evidence.
> `visualAccepted=false`, `accepted=false`
> and `globalParityPercent=null`: complete actions, UI/VFX, authenticated live
> WSS, 100/125/150% DPI, native 30-minute soak, human visual/feel acceptance
> and formal publisher signing remain open.

> 2026-08-27 VIS-00 code gate: the independent visual-parity branch now has a
> source-bound Phase-A ledger plus tested repairs for Arial routing,
> 8pt chat/nameplates, outlined ordinary NameView labels, remote
> normal/Transform body routing, Harvest/CWeapon-01/Skeleton transitions and
> Hidden/corpse opacity. HUD/damage typography, hover corpse names and additive
> weapon/wing layers remain open. This entry has no same-EXE
> capture and does not supersede the currently running frozen Candidate.
> Focused gates pass; the full suite remains FAIL because the frozen asset pack
> is missing the Archer walk and Mount frames required by two of 318 Windows
> tests. Treat VIS-00 as an
> implementation checkpoint only, with `visualAccepted=false`.

> 2026-08-19 alternate-class/combat-overlay update: the native resolver now
> mirrors the Web alternate libraries for Archer walk/run/range actions
> (`ARArmour` / `ARHair` / `ARWeapon`) and Assassin body/hair plus directional
> dual-weapon actions (`AArmour` / `AHair` / `AWeapon*`). Combat presentation
> now treats Crystal `ObjectStruck` as the pose packet and the separate
> `DamageIndicator` packet as the authoritative numeric hit/miss/crit/heal
> event. Floating events are deduplicated, bounded and animated. Native F-key
> targeting also ranks active-quest match, then live distance, and only uses a
> stale `selectedObjectId` as a same-distance tie-break. A live Deer changed
> `10/25 -> 9/25 -> 8/25`; the renderer-owned F12 frame visibly contains the
> red `1` damage floater at
> `generated/player-qa/native-windows-combat-overlay-round4/windows-native-combat-authoritative-round4-in-game-1787122701585-1.png`
> (SHA-256 `0E24F11B963382F02C82F0DAEEE745F51794A01460175E92523AABE7DCBD49AA`).
> Windows tests pass 104/104, shared runtime 133/133, the full Simulation lib
> passes 1183/1183, Web typecheck passes, and Release SHA-256 is
> `B6A7078173865DF3415B089DE4119EAA438886EF518AFD9DEC69054B445773D9`.
> Starter entity metadata contains no usable `maskPath` or nonzero shadow data,
> so shadows/effect masks require an exporter/asset-pipeline slice rather than
> a renderer flag. Spell/projectile effects, lighting, exact text and final
> same-scene/Gemini/human acceptance remain open.

> 2026-08-19 entity-composition update: Windows now loads 697 usable Crystal
> per-library frame-set catalogs from `original-ui/frame-sets.generated.json`
> and applies the source `start/count/skip/interval/reverse` data with explicit
> action fallbacks. Player rendering is a stable native composite of body,
> hair, front/rear weapon and mount layers; mounted actors suppress weapon
> layers. Native-only entity overlays now render Crystal-positioned NPC/monster/
> player labels, dead-state lines and the self HP bar from the authoritative
> payload. Live keyboard movement moved the same player from `288,615` to
> `287,613` while the camera, minimap, actor and label followed. Evidence is
> `generated/player-qa/native-windows-overlay-round1/windows-native-overlay-round1-in-game-1787118632558-2.png`
> (SHA-256 `343B4B05A9E67EF7B687F0DFA9B5D8D2F34E222A7AD95BC03AD6D1E4569E8DC4`).
> Windows tests pass 98/98, shared runtime 133/133, Release SHA-256 is
> `34B5053B58C2808FF5B7DD7ACE4FEE1721817BDE5977E7DE363EDA76F9B6A738`,
> and Web typecheck passes. This closes the per-library frame-set and basic
> composite/nameplate/HP subgates, not full entity parity: alternate class
> libraries, shadows/effect masks, combat effects, exact GDI/bitmap text,
> lighting, same-scene review and final human acceptance remain open.

> 2026-08-19 entity-animation update: native entity rendering now has a
> Windows-main-thread Crystal animation clock instead of selecting one static
> frame only when a Gateway message arrives. Authoritative packet hints cover
> self walk/run/death/revive and ObjectWalk/ObjectRun/ObjectAttack/
> ObjectRangeAttack/ObjectMagic/ObjectSpell/ObjectStruck/ObjectDied/
> ObjectRevived. Monotonic action sequences prevent repeated snapshots from
> restarting a cycle; monster run/range/spell normalize only to actions its
> current audited default catalog supports. Two F12 captures 254 ms apart are
> `native-windows-animation-round1/...-1787116494338-2.png` and
> `...-1787116494592-3.png`; they differ in 5,087 pixels, with the RGB difference
> bounded to world rows 60..532 and no HUD change. Live TownRevive then restored
> `18/18` at `288,616`, and `W` moved the authoritative map/HUD to `288,615`;
> evidence `...-1787116615991-4.png` has SHA-256
> `853B4E9502EB789EEDF6AC6EC3D2CD78C32A2351E668C7A98DBE252227E769FB`.
> Windows tests pass 90/90, shared runtime 133/133 after aligning its stale
> Windows dependency lock, Release builds, and Web typecheck passes. This closes
> only the per-frame/action-clock subgate: per-library frameSet metadata,
> class/equipment composite layers, overlays and final Crystal visual acceptance
> remain open.

> 2026-08-19 map/entity/vitals update: the native packet-first path now moves
> the authoritative terrain camera, minimap coordinates and HUD together; the
> entity producer reads the schema-v2 multi-page atlas with per-page routing and
> Crystal source offsets; and the retained map renderer rebuilds a page layout
> when camera movement exposes rects not present in the first viewport. This
> removes the 96x64/192x128 black terrain holes. More importantly, shared-Zone
> `ObjectHealth`/`Death` packets now override the lagging personal snapshot for
> both the native HUD and self entity. A persisted dead character visibly opened
> at `HP 0/18`, `V` issued the real `TownRevive`, and the client returned to
> `0 @ 288,616` at `HP 18/18`; subsequent field damage was immediately visible
> as `18 -> 17 -> 16`. Evidence is in
> `generated/player-qa/native-windows-map-layout-round1/` and
> `generated/player-qa/native-windows-vitals-round1/`; the near-reference frame
> is `windows-native-vitals-round1-in-game-1787114471473-3.png` (SHA-256
> `E3E0AEF851A90ECD1FC31FEE794DBB34A7BAA2B7A5B34465F3B53667CD56B68C`).
> Gates pass at Windows 83/83, shared runtime 133/133, Release build, Web
> typecheck, and Git Bash package-script syntax. A full offline asset staging
> run also passed: 8,325 files / 269.91 MiB were copied and every required
> manifest, map pack and Crystal UI sentinel was verified. Final same-coordinate
> scene, complete entity action/equipment layers, effects, DPI execution and
> human acceptance remain open. This local staging result does not yet prove a
> clean-checkout package: native-keyed and late ChrSel inputs must be generated
> or tracked by CI, and the EXE must still launch from outside the repository
> with `MIR2_NATIVE_ASSET_ROOT` unset.

> 2026-08-19 visual-parity update: the exact Crystal login screen is captured at
> `generated/player-qa/native-windows-visual-parity-round2/visual-parity-round2-login-1787100667931-2.png`
> (SHA-256 `A108BE789722A619BAFF0473DD3131F15AAC21EC8F5AD0108FA05D93BBC1D9CC`).
> The final structured report is
> `generated/player-qa/ai-visual-review/antigravity-native-login-final-20260819/review.md`:
> Accepted, 100/100, `sameScene=true`, zero visible issues. This supersedes the
> old login frame for visual acceptance only. Exact Crystal character select is
> now captured at `generated/player-qa/native-windows-select-round1/windows-native-character-select-character-select-1787102742999-1.png`
> (SHA-256 `1536F6C37759B85C1B705B746A2ED6E805B5DF078A8BD9F3B7B8B1889A898A85`)
> and reviewed at `generated/player-qa/ai-visual-review/antigravity-native-select-round1-20260819/review.md`:
> Accepted, 100/100, `sameScene=true`, zero visible issues. An occupied-roster
> capture at `generated/player-qa/native-windows-select-occupied-final/windows-native-character-select-occupied-final-character-select-1787103615727-1.png`
> (SHA-256 `F62C413AE599A12F662EAF99CA644947ACDA96B4A90F698FB9B891AB42778FBF`)
> verifies the animated class preview and selected row. The first exact Crystal
> in-game HUD Candidate is captured at
> `generated/player-qa/native-windows-hud-round2/windows-native-crystal-hud-round2-in-game-1787106442919-2.png`
> (SHA-256 `D7A6EBC21EBB56A52EF20A3F5C76A1CF441F1D6C33C1058CF4F8C83B641C578B`).
> The HUD-only evidence pair is reviewed at
> `generated/player-qa/ai-visual-review/antigravity-native-hud-round2-20260819/review.md`:
> Accepted, 88/100, `sameScene=true`, no P0/P1. This clears the first-Candidate
> threshold of 85, but not the final 92 target or final human acceptance. The
> original six-frame table still records the completed functional q1-to-q2 flow.

状态：Windows 原生可玩 Candidate 的实现、自动回归和本机证据齐全；等待最终 Crystal 1:1 人工前端接受。
目标程序：`mir2-platform-windows.exe`（Bevy/Winit/WGPU 原生窗口，不使用 WebView、Tauri 或浏览器 DOM）。
验收视口：1024×768。
权威状态：Gateway + Simulation；客户端只发送意图并渲染服务端回包。

## 1. 固定截图清单

下列文件均已验证为 1024×768 PNG。目录 `docs/generated/player-qa/` 按仓库策略被忽略，属于本机验收产物，不作为源码提交内容。

| 阶段 | 文件 | 可见验收点 | SHA-256 |
|---|---|---|---|
| 登录 | `generated/player-qa/native-windows-candidate/01-login-login-1787076463613-1.png` | 账号、掩码密码、Login/New Account、原生背景 | `4FAECEA3333295CB83FCE7F619451954DD6E499122AD9507168CEE52440CDE2C` |
| 选角 | `generated/player-qa/native-windows-candidate/02-character-select-character-select-1787076504781-1.png` | 服务端角色、职业/性别/等级、创建、开始、退出 | `3A32D8E2F384555953C78AE4DA6C67E1C78669E2BFC68E846C08132FA018F8AD` |
| 进图 | `generated/player-qa/native-windows-candidate/03-in-game-in-game-1787079477140-1.png` | 比奇省地图、实体、HP/MP、目标、任务、背包、原生控制提示 | `02A39AA420C5BFC1A1FB0822759D26862123D7BEAD1CB91C89DAAC1A` |
| 已接任务 | `generated/player-qa/native-windows-candidate/04-quest-accepted-quest-accepted-1787079731634-1.png` | 任务 2 `In Progress`、GingerTea 0/1、CraftsLady 邻近提示 | `F9253A68A819F847CC2B1241AF8A7DD0537DD4CAF4F5FA2F7B13FB79888C2103` |
| 战斗/进度 | `generated/player-qa/native-windows-candidate/05-combat-combat-1787080889965-1.png` | 目标 12/25 HP、任务 `Ready to Turn In`、GingerTea 1/1、背包物品 | `F725A2C00A999DC0074E5574B7B39B3B88F8DC94A894CDA15FADC63479CB0445` |
| 已完成 | `generated/player-qa/native-windows-candidate/06-quest-complete-quest-complete-1787081418998-1.png` | 任务 2 `Completed`、200 Gold、30 EXP 与奖励说明、奖励物品 | `B3CD2F848D84F52775AC135A9FA6E27538259E9936A2C014D9EDF4804B74A60A` |

## 2. 人工 E2E 操作清单

人工执行时不要设置自动截图状态变量，也不要使用 `demo`、GM、QA、传送或直接修改存档。账号和密码应由玩家在原生登录页输入。

| 步骤 | 玩家输入 | 预期权威证据 | 预期可见结果 | 对应截图 |
|---|---|---|---|---|
| 1. 启动 | 双击/运行原生 EXE | WebSocket 连接成功；尚未发送 Login | 先出现原生窗口和登录页，空凭据也不会卡住启动 | 01 |
| 2. 登录 | 输入账号、密码并按 Enter/点击 Login | `LoginSuccess` 携带角色数组 | 密码只显示掩码；失败可重试；成功进入选角 | 01、02 |
| 3. 创建/选择角色 | 创建 Warrior/Male 或选择已有角色，点击 Start | `NewCharacterSuccess`；随后 `StartGame(result=4)`、`UserInformation` | 使用服务端 `characterIndex`；收到玩家世界初始化后才进入游戏 | 02、03 |
| 4. 原生移动 | WASD/方向键，Shift+移动测试跑步 | `UserLocation`；AOI 对其他对象发 `ObjectWalk/ObjectRun` | 玩家和摄像机移动，地图碰撞有效；失败移动得到校正 | 03 |
| 5. 完成任务 1 | 靠近 Jane，按 T 交互，接取；移动到 CraftsLady 并交付 | `NPCResponse`、`NewQuestInfo/ChangeQuest`、`CompleteQuest` | 任务 1 完成并解锁任务 2，经验增加 10 | 03、04 |
| 6. 接取任务 2 | 与 CraftsLady 交互并接受 | `ChangeQuest` 显示任务 2 `InProgress` | 任务追踪显示 `Collect GingerTea (0/1)` | 04 |
| 7. 战斗 | 移动到 Scarecrow，选中目标，按 F/Attack | 每次有效命中有 `ObjectHealth`；死亡有 `ObjectDied`；击杀奖励有 `GainExperience` | 目标 HP 只随服务端回包下降，死亡/失败攻击不由客户端伪造 | 05 |
| 8. 任务物品 | 等待合法 Q-drop 结算；普通地面物品可按 R 拾取 | Quest 2 的 Crystal `Q` 项通过 `GainedItem(item_index=1112)` 直接进入任务包；普通地面物品使用 `ObjectItem` + `PickUp` | GingerTea 变为 1/1，任务进入 `Ready to Turn In`；普通地面物品出现时 R 可拾取 | 05 |
| 9. 死亡恢复 | 若途中死亡，按 V | `TownRevive` 后收到 `Revived/ObjectRevived`、`UserLocation` 和正 HP 快照 | 返回城镇复活点，HP 恢复，继续任务，不保存死亡前陈旧坐标 | 05 |
| 10. 交付任务 2 | 回到 Jane，按 T，选择完成任务 | `CompleteQuest` 包含任务 2；交付增量为 30 EXP、200 Gold、GoldenPendant、CopperRing | 任务显示 `Completed`，奖励和金币可见，GingerTea 被消费 | 06 |
| 11. 重登 | Logout，再次登录并进入同一角色 | 新会话的 `UserInformation/worldSnapshot` 与保存结果一致 | 坐标、HP、经验、200 Gold、奖励物品和任务完成状态保持；不能重复领奖 | 06 |

## 3. 自动权威闭环

脚本：`apps/game-client/platform-windows/scripts/smoke-native-flow.mjs`

2026-08-19 使用全新账号和全新角色的一次执行结果为 `ok: true`、退出码 0，覆盖：

- 新账号、登录、建角、StartGame 与真实世界初始化；
- 任务 1 接取、移动、NPC 交付；
- 任务 2 接取、27 个权威步走到战斗区；
- 同一 Scarecrow 从 100% 到 0% 的 20 次有效攻击回包；
- `ObjectDied`、30 击杀经验与 `GainedItem(item_index=1112, count=1)`；
- 玩家死亡后的真实 `TownRevive` 生命周期；
- Jane 交付、30 任务经验、200 Gold、GoldenPendant、CopperRing；
- Logout/Login/StartGame 后位置、经验、金币、背包和两个任务完成状态一致。

注意：Crystal `Q` 标记物品不是普通地面掉落。共享 Zone 的生产实现会把它直接送入合资格玩家的任务包；验收脚本因此要求真实 `GainedItem`，不能错误要求 GingerTea 一定先成为 `ObjectItem`。普通地面拾取仍由 `PickUp/PickUpTile`、Gateway 事务和 Simulation 测试单独保护。

## 4. 当前视觉限制

此证据包证明“Windows 原生且可玩”的垂直闭环，不代表 Crystal 画面已经 1:1：

- 多页 entity atlas、source offset、697 个逐库 frameSet、基础 body/hair/weapon/mount 复合、Archer/Assassin alternate class library、名字/自体血条和权威伤害飘字已接入；源导出仍没有可用的 `maskPath`/shadow 元数据，技能/投射物特效、精确 GDI/位图文字和高优先级动作打断仍未完整收敛；
- HUD 已迁移为 Crystal MainDialog/ChatDialog/快捷栏/小地图，首轮结构化审阅 88/100；最终 92 分门仍需位图字体和同场景整屏复核；
- 靠近 `0 @ 257,594` 的实走证据已到 `261,595`，但动态实体/控制状态阻断了最后 5 格；不能把该帧伪称为精确同坐标，需先固化可重复的实体布局或安全验收会话；
- 任务文本来自原始数据，部分源数据中的空占位或英文拼写问题仍会原样出现；
- Windows 125%/150% DPI、长时间运行、断网重连和发布包离线资源仍需最终人工矩阵确认。

这些限制应进入 `FRONTEND-1TO1-GAPS.md`，不能用本次“可玩闭环通过”替代最终 Crystal 1:1 人工接受。

## 2026-09-09 trade visual rerun

Click-select/place partial merge, chosen-slot retrieve, occupied trade swap and empty trade move passed native visual checks with successful server ACKs. Continuous drag did not dispatch; full-consumption/directional/lock/cancel visual cases remain open. See [scoped evidence](generated/player-qa/windows-trade-visual-20260909/README.md). Candidate/global visual acceptance remains false.

Drag-only follow-up: one native drag MoveItem Trade 9 -> 4 succeeded after repositioning the panel, but adjacent attempts did not dispatch and one selected the destination. Drag is intermittent under Sky input; acceptance remains open. See the scoped evidence follow-up.

## 2026-09-09 native trade drag fix

The intermittent item drag is fixed by replaying ordered Winit/Bevy WindowEvent input and retaining a preselected source until release. Pointer tracking survives pending/modal gesture cancellation. Native UI 624/624 and Windows host 561/561 serial pass. Exact signed EXE B62330DFA03EB8A2A5976C0662A327D95A2E65A4721C0B3E2D0ECC3F9DE75599 passed seven native drag operations: deposit, occupied swap, preselected-source move, retrieve, redeposit and partial merge, with matching ACKs. This supersedes the prior drag-open finding only; full-game acceptance remains false. Evidence: [drag fix](generated/player-qa/native-trade-drag-fix-20260909/README.md).
