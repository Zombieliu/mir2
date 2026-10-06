# Web / Windows 固定基线追赶 QA

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
