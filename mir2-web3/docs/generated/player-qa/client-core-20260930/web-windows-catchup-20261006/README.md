# Web / Windows 固定基线追赶 QA

Windows比较基线为`f72e36fb84c3574fff0aeb2abed856454b14289d`，原Web审计父提交`f1cf96324c7da62e57d5fd0fa4146be010adbba5`；本轮Source18父提交`2e574054a7734fa75efea9378e831a8ee476cc8a`。本页区分源码、有限代码检查、静态构建和玩家行为；没有运行服务、HTTP、WASM实例、浏览器、原生窗口、真实账号或存档。Goal仍active，Candidate及可玩性未验收。

当前Web包：[.mir2-thin-client-web-windows-catchup-20261007-01](E:/mir2-player-journey/mir2-web3/apps/web/.mir2-thin-client-web-windows-catchup-20261007-01)。体积372,680,891 B（355.42 MiB），7,301文件、776目录（含根目录；不含根为775）、0链接，原360 MiB预算余4,806,469 B。对应Source18，Core/PUI四个和renderer六个发布文件与Next编译manifest精确匹配，未启动。

## 当前Source18地图、聊天与Cash批次

[有限检查](presentation-source18-finite-result01.json)与[五阶段组合构建](presentation-source18-combined-build-result01.json)分别保存实际结果和产物闭包。大/小地图图像寻路、聊天历史拖动、4/7/11行、settings Apply/Cancel/Defaults和Cash sprite预览/转向九条已成为源码候选。Native/Web共用Rust规则与控制器；Web继续使用DOM绘制，未计为共享painter。纯修理报价已提取，普通/特殊维修的真实Bag UID及NPC rate接线仍未闭合。移动端可复用独立presentation-ui轻量包，无需依赖Bevy renderer；移动真机行为未验证。

选定Rust跨组212次通过、3次ignored，仅按逐项有效输入图承接：Source16 Core85、Source17默认ABI44＋PUI ABI10，以及Source13 Native聊天32/商城11/报价6/UI-core24；不是212个独立场景，也不是Source18全部新执行。Node05实际11脚本218/218、0失败/0跳过；Stage5的276内部组只贡献一个文件级Node测试。TSC04严格非增量编译退出0。历史有限/TSC/Core失败保留在结果中。

Core05、Native01、Renderer01、Next01与Thin01五阶段实际构建通过，原CargoGuard和各项预算保持。Core固定-Oz后WASM252,205 B/JS24,393 B，独立PUI WASM70,277 B/JS14,472 B，各包仍满足WASM严格小于262,144 B和JS≤204,800 B；四次旧单包超限保留。Windows开发EXE104,710,144 B已归档，未启动。三renderer版本`bevy-984e8f644cf26319`，原WASM/gzip/JS上限保持。

Next01严格TypeScript与13静态页通过：19,007 regular产物、61 NFT/37,094 unique引用（37,092文件＋2目录）且0缺失。原source node_modules junction只作为声明来源，不遍历链接；Thin包完整7,301个文件hash已核对、0链接，四个Core/PUI和六个renderer叶文件与编译manifest一致。独立只读源码及静态产物复核零确认P0/P1，不证明启动、资源请求、玩家或移动设备行为。44条warning内容及重复次数和Source12上一包相同，实际运行语义仍未验；231份JSON当前source/output全部逐对字节相同，仅在此条件下承接旧token保持结论，本轮未新跑parser/lexer。远端资源miss仍需配置immutable origin，Pet/Gate完整库存和覆盖未验。

[当前矩阵04](feature-matrix04.md) / [完整JSON](feature-matrix04.json)保留317稳定ID、原始审计、Native证据、overlap与unknown：103 shared、197 legacy、9 open、8共同限制。九条新增候选从open转为legacy，不以共用规则代替共同绘制或行为验收；190具名fixture行关联/104独立声明不等于实际执行或玩家行为。全部317行player为not-run，整体验收分母和百分比仍为null。当前指纹见[索引04](current-evidence04.json)。下一轮真实普通/特殊修理与报价，其后Bag/Belt tooltip与Bag→Belt、钓鱼、Pearl和Ranking Inspect；未审范围和共同限制继续保留。

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
