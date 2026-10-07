# Web 对齐 Windows：固定基线目标

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


## 历史Source19阶段状态：维修行为仍partial/open（2026-10-07）

Source19 Node有限检查218/218、strict TSC通过；Next01和Thin01静态构建通过。本批未新建/执行Rust、native、Core或renderer构建。维修Page与NpcShopWindow已接真实Bag UID、完整tooltip来源、NPC rate/owner、共享Rust报价、proof和回执屏障，但未接固定Windows流程中的Bag拖拽目标与Hold自动确认；三个F09维修行仍open/partial。矩阵05仍317稳定记录、103 shared/197 legacy/9 open/8共同限制，player全部not-run，不冻结完整分母或整体百分比，不宣称Candidate/goal完成；W6实际UI/玩家操作依用户要求暂缓。见[矩阵05](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/feature-matrix05.md)和[QA索引](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/README.md)。

用户于 2026-10-06 要求以新的 goal 开始落地。新目标已实际创建为 active；本文件记录本轮范围与交付状态，不把旧目标的 blocked 状态沿用为当前状态。

## 固定基线与交付范围

- Windows / 共享源码基线：`f72e36fb84c3574fff0aeb2abed856454b14289d`，tree `85294233bc2c0356f3acc93c5e8b45d2816fe18b`。
- 目标：Web 普通入口的功能及共享交互逐项对齐这份 Windows 源码，并交付与当前源码对应的候选包。基线代码存在不等于其行为已经验收。
- 共享规则、控制器和表现继续复用；平台适配只处理浏览器输入、资源、渲染和传输边界。
- 两端共同缺陷、Crystal 全项目差距和移动真机验收单列，不把它们当成 Web 相对欠账。必要的共同修复仍可实施，但优先收尾 Web 对齐路径。
- 沿用用户“继续代码，暂不操作界面”：不启动或操作浏览器、原生客户端、真实账号或存档，不运行实际玩家流程。不查询或触碰受保护原生 PID 34412 / 51792。允许的源码、有限测试、类型检查和构建继续推进。
- 不部署生产，不读私有环境或账号，不清理无关缓存、产物或玩家保存数据。重建复用现有构建工具和预算；不增加 QA 框架或执行工厂。

## 首轮工作清单

| ID | 工作项 | 代码 | 构建 | 实际玩家验证 |
| --- | --- | --- | --- | --- |
| W0 | 固定Windows功能清单，逐项核对共享实现、旧入口及差距 | 317条有界记录与9类更正；46个基线blob实际匹配；Source21后仍有4条已确认代码差距和未审范围，分母不冻结 | 源码分类与构建证据分开 | 全部待验证 |
| W1 | 支持的桌面路径默认请求共享任务/HUD/角色/技能/邮件/仓库/普通商店与背包，保留显式关闭和 readiness 降级 | 默认请求接线完成；Node 22/22、严格 TSC 通过 | Source09历史构建保留；当前Source18五阶段build-only通过；Native09 / renderer07 / Next03 / Thin05 build-only 闭合，行为待 W6 | 待验证 |
| W2 | 九类任务动作接入共同 controller / Web host，不新增客户端世界权威 | 九类均有代码候选；Rust Source02 27 distinct 测试和 4 项编译/检查闭合，见本轮状态 | Source09历史构建保留；当前Source18五阶段build-only通过；Native09 / renderer07 / Next03 / Thin05 build-only 闭合，行为待 W6 | 待验证 |
| W3 | 常规 WebGPU / WebGL2 入口选择共享表现，固定首屏 canvas / ABI / package / 单次启动关系 | Node 155/155、严格 TSC、Rust 启动/画布 10/10 通过；可选依赖修复回归通过，三套 WASM 编译检查退出 0 | 已纳入 Source09 当前候选包；renderer07 / Next03 / Thin05 build-only 闭合，运行时与行为待 W6 | 待验证 |
| W4 | 对齐功能、输入、资源及生命周期 | 原七项W4b、配偶与认证候选保留；地图/聊天/Cash九条新增源码候选已通过有限检查；修理、tooltip、Bag→Belt、钓鱼、Pearl与Ranking继续 | Source18当前五阶段静态构建通过 | 待验证 |
| W5 | 重建源码对应Core/renderer/Next/独立包并保持预算 | Source18限定选定Rust212次通过/3ignored、Node218/218及严格非增量TSC；两个轻量包/Native/三renderer/Next/Thin实际构建通过 | 当前Thin01 7,301文件、776目录含根、372,680,891 B、0链接；原360 MiB cap余4,806,469 B；44 warning保留，运行语义未验 | 待验证 |
| W6 | 登录→任务/战斗→保存→重登及平台间行为比较 | 不以夹具替代 | 不以编译替代 | 用户暂缓界面操作 |

W0 审计闭合前不报告整体百分比。此表是执行阶段，不是功能数量或完成率分母。F11 原 44 项行为分母尚未闭合，不冻结。最终功能矩阵将独立列出当前实现、有限检查、当前构建及实际流程证据；两端源码都有功能不能直接勾选“已可玩”。


## 当前状态快照（2026-10-07）

Windows固定基线f72与原Web审计f1cf保留317条有界源码记录：103 shared、197 legacy、9 open、8共同限制；不是完整验收分母，不报整体百分比。Source18补齐大/小地图图像寻路、聊天拖动/4-7-11行/settings draft和Cash预览/转向九条源码候选，规则与控制器共用Rust，Web绘制仍为DOM。选定Rust跨组212次通过、3次ignored仅按有效输入限定承接；11个有限Node脚本218/218、0失败/0跳过及严格非增量TSC通过。Core/PUI两个轻量包、Windows开发EXE、三renderer、Next严格TypeScript＋13静态页及Thin均已实际构建；独立包7,301文件/776目录（含根）/372,680,891 B/0链接，原360 MiB cap余4,806,469 B。源码、输入及静态产物独立复核零确认P0/P1。44条warning内容与重复次数保持，231份JSON仅在source/output逐对字节一致条件下承接历史token结论，未新跑parser/lexer。真实服务/HTTP/UI/WASM实例/账号、玩家流程及移动真机均未验，goal active。见 [行为矩阵04](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/feature-matrix04.md)、[Source18组合结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/presentation-source18-combined-build-result01.json) 与 [QA索引](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/README.md)。

历史配偶批次：Mail仅开启具名本地草稿，Whisper仅填入Native精确文本`:)`；名字/map、物理owner、scene/map、Bonds epoch/open及完整关系投影在捕获与执行时复核，开Mail后再复核，不自动发送。配偶Online/Offline展示与Whisper可用性共用收到的map。当前四文件独立review02零确认P0/P1；这是源码审查，玩家流程仍待验。

历史Thin05原样携带Core版本`c4952ff102f1fe1a35cc48b456e68adb33ede425f657d75fa25e6b811e6f979e`及renderer`bevy-e31f4cb651a7b4ef`；全包文件hash核对通过，3,366公共文件与Thin03字节相同，79 atlas PNG相同。231 JSON的原token保持证据只在当前source/output逐对完全相同的条件下限定承接，未新跑parser/lexer。44 warning内容和重复次数与Thin04相同，28源码pin复核相同；编译config仅distDir/distDirRoot变化，不能据此豁免RSC/构建分支/native加载的真实运行检查。

历史Source12：F01七条已完成当时源码、有限检查与组合构建：[Source12认证结果](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/auth-source12-combined-build-result01.json)。Native/Web共用注册、改密与SafeKey规则；Web表单仍为DOM候选，未宣称共同painter。Core252,167 B保持原cap，余9,977 B。Source18已补齐地图、聊天和Cash九条源码候选；当前下一轮为真实修理接线与物品tooltip/Bag→Belt，钓鱼、Pearl与Ranking继续排队。真实UI/WASM实例/账号、完整玩家和移动验收仍未进行；历史失败、旧Source及旧包回执全部保留，F11原44项分母未恢复/未冻结。

## 固定源码功能清单：11 组初查

针对基线进行源码存在性与接线初查；全部实际玩家比较尚未执行。后续细分行为用例后才冻结验收分母。下方九类动作明细最初记录实现起点；其待接线状态已由上方当前快照更新为代码候选。

| ID | Windows 功能范围 | Web 当前来源/入口与待比较项 |
| --- | --- | --- |
| F01 | 登录、角色创建/选择、进入游戏 | Web session/character 流程已有；当前构建完整流程待比较 |
| F02 | 地图、移动、目标、场景交互 | 初查时记录：共享 map/combat、Web movement shadow、任务快捷入口及 canvas 默认表现待统一；当前默认 GPU/GL2 共享表现已为代码候选，实际移动/场景流程待比较 |
| F03 | 任务接取/提交、引导、导航、补给 | 初查时九类 portable 动作受阻；当前九类均有代码候选并有有限检查，实际任务与导航流程待比较 |
| F04 | HUD、聊天、小地图 | 共享 HUD host 及旧 Web HUD/chat/minimap；全树与实际交互待比较 |
| F05 | 背包、腰带、装备、物品操作 | 共享 inventory/equipment 与 Web bag/character host；兼容操作、布局、生命周期待比较 |
| F06 | 角色、技能快捷键、战斗、反馈 | 共享 character/spells/combat proof，旧 Web hotkey/action；实际目标/技能/效果待比较 |
| F07 | 邮件列表、阅读、写信、附件 | 共享 mail controller/painter/dispatcher 及旧窗口；当前生产包和完整服务流程待比较 |
| F08 | 仓库存取、密码、扩容租赁 | 共享 storage model/host 及旧 password/storage；密码/租赁兼容表面待细分 |
| F09 | NPC 买入、卖出、修理 | 共享普通买入及旧 sell/repair；特殊目录和表面待统一，共同 ACK 缺口不算 Web 独有 |
| F10 | 设置、帮助、快捷键说明 | Web 旧设置/help/hotkey；共同偏好/输入和不同界面待比较 |
| F11 | 社交、交易、市场 | 共享 social/trade 模型及旧 Web 窗口；完整共享行为和服务待比较 |


## W4b：初查确认的 Web-only 差距与候选接线（partial，不冻结分母）

下表保留初查核实的七个普通入口差距作为历史对照；当前七项均已有代码候选。对应 TSC/finite/Next 检查已在 Source09 冻结输入上通过；实际 UI 仍未验收，也不构成 F11 或全项目分母。

| # | 行为差距（初查） | Windows/共享源码入口 | 初查差距与当前 Web 候选状态 |
|---|---|---|---|
| 1 | Options 七类持久开关、Observe 权威请求及完整音量入口 | `apps/game-client/ui-core/src/state.rs:122-134`、`registry.rs:730-740`；`options_volume.rs:80-173` | `original-client-audio-settings.tsx:19-100` 初查仅有浏览器本地 music/effects 与音量控件。现已有七类选项、Observe 与音量的 Web 代码候选；最终权威往返和行为待验证。 |
| 2 | Help 45 页内容与分页 | `apps/game-client/client-bevy/src/crystal_ui/overlays.rs:209-352,12900-13182`（45页目录/翻页渲染） | `original-client-help-window.tsx:25-30,141-162` 初查仅五分类。现已有 45 页内容/分页的 Web 代码候选；完整映射待最终检查和行为验证。 |
| 3 | Keyboard 改键、恢复默认与持久化 | `apps/game-client/client-bevy/src/crystal_ui/keyboard_dialog.rs:62-`、`keyboard_dialog_host.rs` | `original-client-hotkey-window.tsx:18-23,51-89,94-111` 初查为只读默认映射且 ALT 标签不符。现已有改键、恢复和持久化的 Web 代码候选；最终映射与持久化行为待验证。 |
| 4 | Ctrl+I/C/S 关闭当前英雄管理页或切换状态 | `hero_dialog.rs:248` `toggle_page`；Native dialog actions | `apps/web/app/page.tsx:8606-8616` 初查快捷键只打开页面。现已有 Ctrl+I/C/S 与英雄管理开关状态的代码候选；toggle/close 语义待最终验证。 |
| 5 | Hero belt 键位使用后的 7/8 restock 行为 | `hero_dialog.rs:46,196-225` restock 状态；Native Hero belt action | `apps/web/app/page.tsx:8615-8620` 初查将 7/8 映射为普通 use。现已有 7/8 与英雄 belt restock 的代码候选；库存状态转换待最终验证。 |
| 6 | Hero Status/State 两个统计页 | `hero_dialog_render.rs:466-467,507-553`；`character_stats::lines` | 初查缺页；现有 `original-client-hero-management-window.tsx` 的 Status/State 页及 `page.tsx` `readHeroStats` 共享只读 getter 接线。代码候选已接，对应 TSC/finite/Next 已通过，实际行为待验证。 |
| 7 | Floating Hero 2-slot belt 的关闭与旋转 | `hero_dialog_render.rs:315,672-778` (`CloseInventory` / `BeltRotate`)；`hero_dialog.rs:53,61-62` | 初查缺控件；现有 `original-client-hero-management-window.tsx` 已接 belt visibility/orientation，经 `closeWindow("belt")` / `onWindowChange` 更新窗口。代码候选已接，对应 TSC/finite/Next 已通过，实际行为待验证。 |
主要入口：`platform-windows/src/main.rs` 注册 Native Shell/HUD/chat/Quest；`runtime/src/quest_ui_host.rs` 安装 portable Quest/HUD/Bag/Character/Spells/Mail；`runtime/src/lib.rs` 安装 combat/storage/NPC host；Web `page.tsx` 提供相应模型和现有命令适配。旧 Web 功能存在不等于共享界面已覆盖。

### 行为清单补充与范围核对

只读初查已细分约 40 项玩家行为，但这仍不是完整验收分母。任务接取、交付、奖励、放弃和分享已有 Web 代码链；九类 native-only 按钮仍是明确的共享入口差距。放弃和分享必须保留在完整任务流程中。普通攻击和拾取已有旧 Web 路径，补的是共享任务入口及其校验。

以下补充表记录初始源码入口。W4 后续把相应 Web 动作接入为代码候选，但真实流程仍待验证；源码存在或有限测试通过不等于服务闭环。

| 补充范围 | Windows/shared 源锚 | Web 对应入口 | 后续核对 |
| --- | --- | --- | --- |
| 组队邀请、踢出、离组和邀请开关 | `ui-core/src/action.rs` / `crystal_ui/group_dialog.rs` | Page `group` 组件与 handlers | 共同呈现、状态及命令回显 |
| 公会成员、公告、聊天和等级 | `crystal_ui/guild_panel.rs` | Page `guild` 组件与 handlers | 完整面板与权限/回显 |
| 公会仓库、金币及物品变化 | `ui-core/src/action.rs` / `crystal_ui/guild_storage.rs` | Web guild props 和仓库命令 | 完整请求、回执及投影 |
| 好友、屏蔽、密语、备注 | `crystal_ui/friend_dialog.rs` | Web friends window | 共同呈现及操作链 |
| 婚姻、导师关系 | `crystal_ui/social_bond_dialog.rs` / Windows `social_bond_wire.rs` | Web bonds window | 当前关系与动作往返 |
| 排名 | `crystal_ui/ranking_dialog.rs` / Windows gateway | Web ranking window | 请求、刷新与回显 |
| 英雄召唤、召回、控制 | `crystal_ui/hero_dialog.rs` / Windows `hero_wire.rs` | Web hero/pet window | 指令和权威状态分别验收 |
| 宠物召唤、释放、拾取模式 | `crystal_ui/creature_dialog.rs` / Windows `equipment_creature_wire.rs` | Web hero/pet window | 命令、装备及状态分别验收 |
| 玩家交易 | `crystal_ui/trade_dialog.rs` / Windows gameplay bridge | Web trade window | 邀请、确认、取消、金币和结果 |

NPC 卖出、修理、设置、帮助、快捷键、社交及交易的旧 Web 路径已存在，其共享表现仍待 W4 核对。普通 NPC 购买 ACK/恢复是两端共同缺口。初查未确认市场、领地/攻城的 Windows 专属面板入口，不能把 Web 旧组件反推为 Windows 已完成，也不能直接记为 Web 相对缺口；旧 Web 功能仍应保留。扩展仓库租赁有 shared 源码和 Web 命令，完整宿主链及玩家行为继续单列。

早期 W4 只读复核曾发现 Web 缺 incoming reply、仓库/交易物品动作与租赁确认；这些现已进入代码候选。当前 tooltip 接线覆盖 Trade、Hero 普通玩家背包、Cash 完整目录预览与 Guild 真实目录实例；不把旧 Native02/renderer01 构建当作当前 Source04 包。真实 UI/服务回执仍未验收。Guild rank/permission maintenance、PlayerInspect 与普通 Gold 购买 ACK 属共同 backend 限制；F11 分母仍未冻结。

## 第一片实际检查

变更只涉及纯默认请求策略及 Page 接线。SSR / 首帧仍 request=false；挂载后无参数默认请求 quest/bag。显式 `=0` 分别关闭，首个重复值、原型模式的严格 raw 编码限制和普通模式的原有解码语义保留。ABI、完整模型、当前身份、场景/布局与 ready 状态继续决定实际 owner handoff。

实际执行：既有 Node 文件 shared-canvas-mode、runtime-package-selection、runtime-build-identity 共 22/22 通过、0 skip，其中三项新增具名测试；严格 `tsc --noEmit --incremental false` 退出 0；`git diff --check` 退出 0。Sol/high 独立只读审查 0 blocker。没有真实 renderer/client/网络/UI 执行，没有新生产包验收。

普通 WebGL2、禁用 runtime 和 touch-first 路径继续按能力/启动/readiness 门槛降级，不把 request=true 解释成 UI 已运行。下一轮 W3 同时对齐 JS package/startup 与 Rust shared GL2 默认值，保留 WebGPU 优先；根据实际 backend 选择 presentation canvas，同时保留 manifest/ABI/capability、单次 boot 和 legacy/two-package 回退。

## W3 代码候选与检查状态

普通三包 manifest 无 shared-canvas override 时，WebGPU 仍优先，WebGL2 选择共享包。JS 与 Rust 均以缺省开启、独立首值关闭的规则请求 UI；显式 raw prototype 仍限定 GL2，legacy / two-package / 禁用和 touch-first 路径保持能力降级。所有共享 UI 使用固定的舞台 quest canvas：GPU 将它作为第二个 Window，shared GL2 将它作为唯一 PrimaryWindow。世界 canvas 永久留在原 world composite 内，不移动已挂接的 canvas，也不改 GPU 世界层级。

新增独立只读 canvas selector getter；shared GL2 在启动前必须确认 selector 为舞台 quest canvas，并同时通过 package/backend/ABI、编译能力、完整 query 签名和单次 boot 门槛。Rust shared 模式必须同时安装 UI host 并关闭世界 camera。Page 同一 render 复核严格 raw 请求，避免 backend 切换的一帧借用普通解码值。共享 GL2 的战斗可见性读取实际 DOM 世界，UI 画布临时隐藏不能关闭仍可见的世界输入。

Sol/high 独立只读生产及测试修复审查未发现阻塞；严格 TSC 退出 0。第一轮 12 个有限 Node 文件执行 155 项，142 通过、13 失败、0 skip：旧前景 owner 夹具缺少 NPC/Storage 回调，部分 AST 断言仍假设两个 canvas 都承载 UI。修复保持原有 pointer、owner、modal 和键盘断言；四组聚焦重跑 31/31 通过，最终完整 12 组重跑 **155/155 通过、0 skip**。原始失败日志保留，不把各轮重复执行相加成独立场景。

Root 已通过既有 immutable .NET CargoGuard 串行执行启动策略 6/6、world camera 3/3 和 shared single-PrimaryWindow 1/1，共 **10/10**。共享 GL2 与 GPU 的 WASM 编译检查已退出 0；lean 首轮实际退出 101：既有 `skill_page_state::normalize_raw_skills` 无条件引用了未启用的可选 `mir2_game_data` 依赖。Source01 六项 child/outer 均已正常退出并 disposed，失败保留完整记录（`w3-finite-checks01.json`）。

有界修复将技能目录补全限定到既有 `portable-quest-ui` feature，保留无 UI 路径的原始字段、空值、名称和快捷键归一化；没有修改依赖或 feature 定义。独立只读审查 0 blocker。Source02 实际执行无 UI 技能回归 **3/3**、共享 UI 配置回归 **2/2**，随后 lean / shared GL2 / GPU 三项 WASM 编译检查全部退出 0。两种 feature 的重复场景不相加为五种独立行为。Source01 的启动/画布 10 项保留原批次来源；本轮没有将它们冒充为 Source02 重跑。

每次 Cargo 使用 fresh actual C ≥ 50 GiB 和 Policy B；Source02 五项 child/outer 均已正常退出并 disposed。两轮分别固定 334 个编译输入，执行前后均一致；Source02 关闭后才释放 W2a 写者。因此这些结果证明冻结的 W3 输入，不代表随后修改的 W2 当前树已经编译通过。实际记录位于 `C:/mir2-cross-platform-storage-20261002/repo-qa-web-windows-catchup-20261006-01`，Source02 记录 `w3-finite-checks02.json` 为 11,316 bytes，SHA256 `7cf5c29f9666035eca8062461f9c0ea4c5d603da8ed85e99800e0a3931598aeb`。当前生产包仍待重建；没有操作真实 UI、renderer、WASM 实例或网络玩家流程。

## 九类任务动作

本轮开始时 portable 任务界面在 `client-bevy/src/quest_ui.rs` 明确拒绝以下按钮动作。以下表格记录该起始状态；截至当前快照，九类均已有代码候选，有限检查证据列于状态快照。部分普通攻击/拾取已有旧 Web 路径；共享入口和上下文适配不代表实际流程验收。

| 动作 | 意图 | 当前状态 |
| --- | --- | --- |
| `ToggleSupplies` | 打开/关闭补给面板 | 代码候选与 Web/portable 有限检查通过 |
| `SelectSupplyVendor` | 选择补给商人 | 代码候选与 Web/portable 有限检查通过 |
| `ShowSupplyInventory` | 查看补给相关库存 | 代码候选与 Web/portable 有限检查通过 |
| `OpenDestinationMap` | 查看任务目的地地图 | 本地 stamped handoff 与有限检查通过，真实地图界面待验证 |
| `NavigateQuestRoute` | 发起任务路线导航 | 代码候选；实际地图流程待验收 |
| `AttackTarget` | 普通目标攻击 | 代码候选，旧 Web 有攻击命令；真实目标流程待验收 |
| `AttackQuestTarget` | 任务目标攻击 | 代码候选；任务目标流程待验收 |
| `PickUpObject` | 拾取指定物件 | 代码候选，旧 Web 有拾取命令；真实场景待验收 |
| `PickUpTile` | 拾取当前格 | 代码候选，旧 Web 有拾取命令；真实场景待验收 |

所有请求仍由现有认证、当前 owner、合法命令、输入和完整模型门槛决定；不得引入 `MoveTo`、debug teleport、原始账号登录或客户端授予成功。

源码核对进一步确认：补给三个按钮操作共享本地面板/商人筛选状态；`ShowSupplyInventory` 切回全部补给摘要，不擅自改为打开背包。导航已有共享任务/map/reset-epoch 校验和单槽队列，但 Web host 尚未接入。攻击/拾取已有共同意图类型，Web encoder 仍拒绝；portable snapshot 还缺库存、地图、当前目标及地面物件上下文。因此不能只移除 native guard 或直接从 JS 发原始战斗命令。

W2b 只读核对确认，`MapInformation` 和 `MapChanged` 协议本来就携带真实 `map_index`，Gateway Web packet 的 `payload.mapIndex` 直接保留它；Page 目前丢弃该字段。`bigMapIndex` 是独立的图片编号，`NewMapInfo` 是按需/搜索缓存，均不能当作玩家当前地图。后续路线适配应补独立当前地图身份，沿用共享 route 校验与单槽队列，交给既有 Web Walk/Run 移动控制器；`MapChanged` 同图重进也必须推进场景 epoch。当前完整 world snapshot 没有 map index，可按 canonical file name 交叉核对已收到的权威 packet 身份，不新增猜测值。

任务攻击需要先关闭/退役任务弹窗，再在同一 owner/scene 的目标仍有效时提交现有 combat controller 的 Select edge，实际发送仍必须经过既有 `CombatProof`。拾取没有同名 proof API，应复用现有地面物件/当前格 handler，并将异步 approach 的目标绑定到 owner/scene，防同图重进的旧 object ID 被复用。上述为源码核对结论，尚未实施或验证。

W2a Source02 的实际 Web 检查为 Node **13/13**、严格 TSC 与 diff check 退出 0，独立源码审查 0 blocker。专用 Quest context 保留完整库存及可空原始技能，不覆盖通用 Bag/HUD/GPU 世界模型；旧 bundle 缺少 exact context version 时剥离新增 DTO。隐藏前即同步拒绝旧输入，恢复后需要新的已应用 revision；技能变化撤销旧完整技能基线。

首轮 immutable Guard 因收据目录未预建而在转发 Cargo 前退出 90，记录保留；创建普通目录后，Source02 Rust 实际编译退出 101、零测试执行，原因是既有 `quest_turn_in` 测试调用没有将 SupplyPlan 包为新签名的 Option。Root 仅修复这一测试参数，生产十文件未变；独立 delta 审查 0 blocker。Source03 固定 404 个输入，实际 portable **5/5** 和 runtime DTO **1/1** 已通过，每次 fresh actual C ≥ 50 GiB、Policy B、guard/child/probe/outer 闭环完整，执行后源 pins 仍匹配。其余 Native 补给与两套 WASM、Native 编译检查继续串行执行；本段不表示新生产包、实际 renderer 或玩家流程通过。

普通目标面板与拾取反馈面板在当前 Native/shared 源码也固定隐藏（`CRYSTAL_TARGET_PANEL_VISIBLE=false`、`SHOW_PICKUP_FEEDBACK_PANEL=false`）。W2b 保留该表现范围，补同等宿主代码能力；任务目标和路线入口按现有可见流程接线。路线必须保留 Native 的猎区 radius 与补给店 radius 2 的区域可达规则，不能以中心格的 `moveToTile` 冒充完整区域规划。

## 模型和文件归属

Root 负责固定范围、平台集成、权限边界、所有实际执行及最终审查。Spark 不在可调用模型列表；按既有用户授权使用 Sol/high 完成有界实现和浏览器启动方案审查，Luna/medium 做只读功能清单。必要的复杂权限或生命周期问题由 Root 处理，不全程使用最高推理强度。

W1/W3 写者已释放并完成有限检查。W2a 原十文件写者已完成；Root 修复类型/完整 DTO/隐藏门槛及一个既有 Rust 测试调用，当前 Source03 十一文件集合冻结。Sol/high 只读审查与 W2b 区域规划探索，Luna/medium 核对实际结果和 W4 接口。Root 独占文档、所有实际测试/构建执行及 Git，同一高冲突文件没有并行写者。冻结检查闭合后才释放下一轮写者。

真实玩家/前端和移动真机门槛保持开放。未完成目标范围所需验收时，不标记 Candidate 100%、已可玩或 goal complete。
