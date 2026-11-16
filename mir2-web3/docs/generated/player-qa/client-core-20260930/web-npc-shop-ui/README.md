# Progress66：Web普通NPC商店共同控件接线

2026-10-04 UTC。本批源码与有限检查通过，整体goal保持active。用户要求继续代码、暂不操作界面；本记录不包含客户端界面或玩家接受。

## 当前行为

普通金币、无限库存、panel0整份目录使用Windows同一Rust商店controller/painter。Web Host传完整原始商品来源、23项PlayerStats、当前service/catalog/source与规范u64十进制字符串Core/control revision；不补造缺失原始字段，不创建token/slot或ACK。

只有完整584×334布局、两块实际输入区域与字体/控件就绪，才交接整棵React Buy树。arming期间旧树可见且inert；实时阻断键盘、场景点击/定时移动、手柄和移动控件。较窄触屏或混合/特殊货币/有限库存/resale目录整体使用legacy。

实际Page sendRaw保持既有listener/wire/receipt顺序；发送前检查当前完整来源及socket，消费精确一次UI proof，最后执行Core entry前checkpoint，随后直接socket.send。旧owner、service、catalog、布局、pointer或同步回调变化不能发送；失败terminal仅释放自身hold，不清除同步新lease。Close/Sell延迟退役；Sell/Repair仍原入口。Flushed/Unknown只表示本地传输观察，不冒充服务器购买成功，不自动重试。

## 选定验证

| 检查 | 实际记录 | 结果 | 最终Source04资格 |
| --- | --- | --- | --- |
| NPC buy/Host/Page-Shell | npc03 | 99/99，无skip | Source03真实执行；两份后改旧测试不在消费依赖内，其余986输入逐字节未变 |
| Storage Host/gateway/Page | storage03 | 48/48，无skip | 同上限定承接 |
| 严格TSC noEmit/incremental false | tsc03 | exit0，无输出 | allowJs=false；后改两份.mjs不在TS/TSX输入内，依赖未变 |
| 仓库真实源码接线 | integration04 | 20/20，无skip | Source04新执行 |
| Bag/identity/equipment/spells/combat | adjacent04 | 82/82，无skip | Source04新执行 |

合计选定Node249项：Source04新执行102、Source03限定承接147；不称Source04全部988输入重执行。NPC99含原buy46及新增53（buy8、Host24、Page-Shell21）。实际Root外层终止exit0，所有选定child completed=true/exit0/signal=null/error=null、sourceDrift=[]。全仓库git diff --check通过，仅已有换行提示。

Rust812依赖图未改，未新执行Cargo；Progress65的62次跨配置有限检查与shared WASM check仅按未变Rust图限定承接，不算本批新增测试。有限Node脚本执行实际纯TS模块与抽取的TS/TSX函数，配合内存边界夹具；不启动完整Page/Shell组件、renderer Instance、WebSocket或真实Core算法。持久Node REPL只做文件/数据/机械编辑，不加载产品或测试。

## 来源与保留历史

QA根目录：`C:/mir2-cross-platform-storage-20261002/repo-qa-web-npc-shop-ui-01`。

- 最终组合Source04：553输入，`root-source-snapshot04.json`，SHA256 `0a5ee4a0df7aeee47c78d1108c8b3e78f19315f043bd36d65b746b9957376c63`。
- WebSource04：988输入，SHA256 `160a3649ef9ae22a159d6757efc9e361cbdf106e78744f4b5738de7dd240c353`。Rust carry812字节未变，联合979个唯一仓库输入。
- 原974输入中965保护、9既有变化；增加既有game-shop及4新Host/hook/测试图输入。写入范围10既有+4新增，完整raw before和反替均记录。Root7产品文件91处机械反替恢复原字节；两份旧测试Source04反替也完整匹配。
- `root-finite-verification04.json` SHA256 `2e5cae21480e5862d79cfa438cc4376066972ee05909de41c93f968a73893585`；原有限runner复用，仅路径/固定NPC argv/timeout0字面调整，未建新receipt工厂。
- Source01未执行；Source02真实NPC21失败和TSC6错误保留；Source03旧Storage4/Adjacent12夹具失败保留。修复实际Host safe回调arity、Shell类型、当前service测试绑定及新增idle NPC依赖；原断言/安全门不放松。
- M14独立复核Source03、Source04 pins、完整反替、严格断言和限定承接条件，执行准入blocker0；最终实际记录与文档仍由Root复核封存。

## 未关闭范围与下一项

生产b17 Core JSON/JS/WASM及现有renderer/Windows EXE/Next/独立包没有因本批源码自动更新；不将旧组合产物继承为包含本批功能。下一项按既定预算，复用有限守卫runner重建Windows、生产Core、三种renderer、Next和独立包。

本批没有UI操作、浏览器或原生窗口启动、HTTP/网络探测、WASM Instance/真实JS sink、截图、玩家登录—战斗—保存—重登、IME/剪贴板或Android/iOS真机接受。普通商店仅源码/有限检查完成；Sell/Repair/BUYBACK/USED共同迁移、无专用ACK恢复、服务端容量/元数据一致性、移动适配、最终frontend及Candidate保持open，goal active。
