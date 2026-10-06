# Android native host ingress audit

## 2026-10-06 完整注册表单的原生宿主接通（完整 goal 仍 Active）

产品源码：`4b21c6c8c367a072b301b11cec4d27b117310133`，父提交 `01442aa505`。
共享 Crystal 注册表单及校验已存在，但 Android 明确拒绝 SubmitRegistration，
Java 宿主也没有入口。本叶只改三个 Android 宿主文件、一个测试文件及本记录，
不改共享 UI 布局、Windows、服务端认证/角色/存档规则；完整 AP-01–21 分母不变。

现在复用共享校验并核对已接受表单/日期 tick，完整转发 newAccount 的 type 与七字段：
accountId/password/birthDateBinary/userName/secretQuestion/secretAnswer/emailAddress。
确认密码与日期文本不发上网；Java 私有控制入口只在 READY 接受，REGISTERING 期间
禁止重复注册、登录、Start 与玩法写入。只按服务端整数结果 8 返回共享 Login，
不自动登录或获得角色/世界权限。范围/形状校验不是 Android 自创的认证规则。
注册结果反馈逐字节沿用冻结 Windows 的展示函数；READY 不再覆盖已处理结果提示。
原生队列满会释放 pending，敏感四字段在入队尝试后清除；发送失败、断线和超时仍终止会话。
预览入口明确拒绝离线注册，保持禁止联网；nativeResumeV1 仍未协商。

失败先行：Java 首轮 compiled 0/14→14/14、Rust 0/8→8/8。
独立只读审查发现 Java 会把 Unicode 数字字符串当作成功，新增 compiled red 1/1，
改为与 Windows 相同的 ASCII 有符号整数解析；最终 GatewaySession 普通版82/82、
预览新增15/15，以及 Rust 新增8/8（两种变体）。测试还直接调用真实旧 socket 回调，
验证新请求 pending 时不能被旧 generation 完成；直接执行真实定时回调，不声称已等满20秒。
协议没有注册 request ID：同连接下一次重试后到达的旧重复回包仍无法区分，未宣称绝对隔离。

450 输入绑定；五文件范围外445整文件、原67个网络/6个Lighting用例正文及原Rust测试模块
逐字节保留。工具连带格式化六个测试文件及旧正文的漂移已全部撤回，未提交范围外改动。
十五个新鲜同源门通过：Android537/preview585、shared1324+10原ignored、
runtime309+1原ignored、Java两种各123（8类，无fail/error/skipped）、API31双检查，
Mac宿主Windows光照14/地图4/效果2/协议21/账号反馈4/Hero FIFO1/技能FIFO1及实际比奇资源1。
计数有交叠，不相加；这是Mac上的参照测试，不是Windows OS全量或人工验收。
首次账号反馈筛选零匹配已保留且排除，纠正模块别名后真实4/4；辅助脚本字段/补丁换行失败
均保留为工具故障，不冒充产品失败或有效验收。

两份 APK 均绑定上述产品提交，Rust release-profile + Android Debug/uiPreview；
code35/name0.1.32-gameshop-phone 未变，不是商店 Release。Gateway为空，预览禁止联网，
GPU诊断开关关闭。Gradle无法strip原生库，按原样封装；字节相同不代表去符号成功。

| 包 | 字节数 | SHA-256 |
| --- | ---: | --- |
| `mir2-native-registration-host-normal-4b21c6c8c.apk` | 533985511 | `482c74db74f6c656b28f02e86457cf18934c8b28703b6d0742dc47992a6694b3` |
| `mir2-native-registration-host-preview-4b21c6c8c.apk` | 542082095 | `d0b9d6bdc63d256736b58b32d4396d819fc8987647f87e1cedbea97dca6ee0e6` |

本机证据目录从 mir2-web3 解析：
`apps/game-client/platform-android/target/native-registration-host-20261006-SAXSZ4/`。
APK在final-apks，publication-evidence.json绑定包/门槛/安装/设备/日志与两张原图。
两包保留数据更新，安装后回读SHA相同；对应variant库产物、6647UI PNG/3metadata、
10个冻结Windows原Lighting帧逐字节匹配。这不是完整获准资源发布或全图覆盖。

API31/Android12/ARM64模拟器，独立8秒前台PID17287/17366，fatal/渲染退出均0，
两张2340×1080原图实际查看。普通包只显示原登录和Test server not configured，
预览是显式OFFLINE/NOT LIVE的本地比奇/对象/夜间灯光；大黑诊断面板及空快捷栏仍在。
普通/预览本次GL506为2/0，诊断记录均0；零错误GPU门仍FAIL，不能靠单次零错误宣布修复。
截图没有执行注册控制写入/注册结果JNI；TLS与Rust状态链不是真实Gateway注册或手机UI验收。

05:19 UTC只读Windows仍56ee063fb，较冻结3d735745f +1提交/83文档/0功能路径。
investigate指导了失败先行、独立复核和五文件边界；同版本wgpu-hal83文件修补、
恢复接线11文件范围以及整套UI三轮失败后的重新批准仍待人类答复，均未擅自实施。
AP-02仍PARTIAL、NI-17 PARTIAL、NI-18 OPEN、完整goal Active；
真实注册/登录/在线玩家/Zone保存、完整恢复、手机UI/多指/IME/九语言、
2969全图缺失引用/完整资源/音频/更新、真机0台与人工验收仍OPEN。
下一步接通其他已有共享账号入口并补获准环境实测；恢复/UI/GPU分别遵守原确认边界。
Status: DONE_WITH_CONCERNS（仅本注册宿主叶）。

## 2026-10-06 实际连接的能力协商修补（完整 goal 仍 Active）

产品源码：`697d6b545e3f61796568c9da1438bea9bed05343`，父提交 `59487ba7a`。
症状/根因：Rust 辅助函数声明了两种能力，但真实 Java socket 只发送 clientVersion；
Gateway 未收到精确商城回执 opt-in。恢复凭据、恢复结果及原生重新加载边界尚未接通，
所以不能上报 nativeResumeV1。通用 gameplay 写入口也未隔离这两种宿主控制命令。
修补只涉及四个 Android 源/测试文件：真实连接在版本包之后发送且仅发送一次
nativeGameShopReceiptV1；新连接重新协商；通用 gameplay 拒绝 clientCapabilities/
resumeSession。Rust helper 与真实连接保持同一已实现能力集合，不改认证或交易规则。

失败先行：新增 Java TLS 五用例 compiled 0/5 → 5/5，Rust 合同 compiled 0/1 → 1/1。
本地 TLS socket 验证了包顺序、每连接次数、未授权游戏输入、控制命令隔离和条件 opt-in
后的精确商城拒绝四元组；它不是实际 Gateway、真实账号、购买成功或实际 receipt JNI 验收。
原 62 个 GatewaySession/6 个 Lighting 用例正文逐字节保留；仅两个 connect helper
增加第二包断言。旧 Rust 能力 oracle 明确去掉未实现的恢复声明，原失败结果保留。
447 输入绑定，443 个非本叶整文件受保护；十四个干净同源门全通过：
Android529/preview577、shared1324+10原ignored、runtime309+1原ignored、
Java两种各108（8类，failure/error/skipped均0）、API31两种，以及Mac宿主Windows
原光照14/地图4/效果2/协议21/Hero FIFO1/技能FIFO1与实际比奇资源1。
计数有交叠，不相加，也不是完整 Windows OS gate。

两个 APK 均 Rust release-profile + Android Debug/uiPreview，code35/name0.1.32-gameshop-phone未变；
不是商店 Release。Gateway为空，preview禁止联网，GPU诊断开关关闭（实测记录0条）。

| 包 | 字节数 | SHA-256 |
| --- | ---: | --- |
| `mir2-native-capability-host-normal-697d6b545.apk` | 533992799 | `5873b81bea5ae47dfb2d52c39ce63c16479187a84a3f7490573d60f62fd99ac7` |
| `mir2-native-capability-host-preview-697d6b545.apk` | 542061335 | `cf1ff6bfb0298a5ecf9e6ef1fc8f580926dca0178a2dfb39385abf405a220fae` |

本机交付目录从 mir2-web3 解析：
`apps/game-client/platform-android/target/native-capability-host-20261006-QrBWwM/`。
APK在 `final-apks/`；`publication-evidence.json` 汇总源码/门槛/素材/安装/设备/原图及哈希。
目标文件均在生成目录，不提交 APK、缓存、密钥或任何真实凭据。
两包保留数据安装并回读SHA一致，各自ELF与对应variant的strip产物一致；
6647 UI PNG、3份metadata、10个原Lighting帧逐字节核验。DEX字符串是辅助证据，
真实包顺序仍以TLS测试为准。首个ZIP全目录输出超限是工具失败，限定DEX条目后通过；
首次普通权限ADB只读EPERM在限定权限下通过，未kill-server/清数据/改全局网络。

API31/Android12/ARM64专用模拟器，两次独立8秒前台PID16665/16743，
fatal/render-validation/AppExit均0；两张2340×1080原图已查看。
正常包虽带lighting-night-jni启动参数，仍只显示原登录及Test server not configured；
预览是明确OFFLINE/NOT LIVE的本地比奇、角色、怪、FireWall和灯光。
旧手机登录目标偏小、大黑诊断HP面板及空快捷格仍未解决，不是UI验收。
正常/预览新PID分别1/5条GL506，零错误GPU门仍FAIL，旧错误保持各自源码归属。

04:01 UTC只读远端：Windows56ee063fb较冻结3d735745f仍+1提交/83文档/0功能；
原工作区Git检查点不变。investigate指导了失败先行和五文件边界（含本文件），
待确认的同版本wgpu-hal83文件修补未实施；此前整套UI三轮失败后的重新批准仍待人类答复。
NI-17 PARTIAL、NI-18 OPEN、完整AP-01–21/goal Active不变。真实登录/在线玩家闭环/
Zone保存、完整恢复、全手机UI/多指/IME/九语言、完整资源/音频/更新、真机0台及人工验收
全部仍OPEN。下一步必须接通完整恢复控制和原生加载/数据边界，不能只打开能力位；
图形修补与UI继续分别遵守尚未解除的确认条件。Status: DONE_WITH_CONCERNS（仅本协商叶）。

2026-10-06 最新诊断源码：`58a92eb7b`（绘制通道）→ `1fbc63f87`（最终屏幕附件）；均默认关闭/显式开关/每进程128条。
两版本各自446输入绑定和十四新鲜适用门通过；其他整文件分别443/444受保护，未改渲染行为或游戏规则。
58版五次新前台启动仍5条GL506，因缺最终输出记录不能判断旧附件假设；1f版五次仍9条GL506，两个失败帧最终附件均为新RBO5。
十张原始2340×1080截图已查看；最终旧指针假设在已观察失败帧被否定，GPU零错误门仍FAIL。
同模拟器独立GLES3 pbuffer三轮对照：删除旧RBO后仅纹理替换3/3 FAIL；保留旧RBO和显式双类型解除各3/3 PASS，像素回读正确。
首轮诊断工具GLES2链接错误未编译，保留且不算游戏失败/有效红灯；上述为正确GLES3链接后的实际九进程。
底层附件类型保留机制已独立复现，不等于正式APK已修复；提出同版本wgpu-hal 29.0.4 Android专用修补，83文件范围待人类确认，尚未引入。
只读Windows来源仍56ee063fb，较冻结3d735745f +1提交/83文档路径/0功能路径；两个Gateway为空，preview禁止联网，真机0台。
[精确两包、两版回归、原始错误、底层对照与待确认修补](generated/player-qa/native-android-final-output-probe-20261006/README.md)。
完整AP-01–21、NI-17 PARTIAL、goal Active及全部未完门不变；旧包/旧实测保留原源码归属。

2026-10-06 历史首帧诊断源码 `951a9ed71`，只读/显式开关/每进程128条，尚未实施渲染行为修复。
同源446输入、443其他整文件保护；新增预算 compiled red3/5→green8/8，十四适用门通过（runtime309含新增8）。
仅新 uiPreview 诊断APK，保留数据安装/回读SHA/原资源核验；五新PID原图已复核，夜间三次共11条GL506，GPU门仍FAIL。
实测把错误收窄到3520×1980→2340×1080首个改尺寸帧、acquire与present之间，发生在离线注入和灯光启用之前。
具体失败绘制附件机制仍待证实，诊断开关会影响时序，不能把零报错样本当验收；下一步追踪绘制目标。
[首改尺寸诊断、精确APK及完整未完门](generated/player-qa/native-android-surface-probe-20261006/README.md)。
完整AP-01–21、NI-17 PARTIAL、goal Active及Windows冻结功能基线不变；下列旧包/旧实测严格保留原源码归属。

2026-10-06 当前原生 Android 光照源码：共享灯源 `c57a3cf06` + 贴图兼容修补 `1f01c1351`。
复用原 Windows 地图/对象/效果灯源，匹配资源请求/render-ready/同帧 pose 后发布；Windows 仅薄委托。
实际 API31 缺少 VIEW_FORMATS 导致首版渲染退出；修补保留原 sRGB 视图，未改 shader/颜色/游戏规则。
精确源码十四门：Android529/preview577、shared1324+10原ignored、runtime301+1原ignored、
fresh Java103+103、API31双检查/Mac-host Windows14+4+2+21+1+1及真实本地资源门通过；不是 Windows OS 全量验收。
双同源诊断APK已保留数据安装/回读SHA，445输入绑定；选定6647PNG+3metadata+10Lighting逐字节匹配。
5次新冷启动均在前台且无渲染退出；12张原图已复核，夜间/黎明/地图黑暗离线画面有地图、角色及灯光。
新5 PID仍8条GL506，零错误GPU门FAIL；全图仍2969缺失引用，非完整资源/全手机UI/联网或真机验收。
只读Windows56ee063fb仍较冻结3d735745f +1提交/83文档证据/0功能路径；NI-17 PARTIAL、AP-01–21完整分母与 goal Active 不变。
[当前同源APK、十四门、原始失败及实际画面边界](generated/player-qa/native-android-lighting-sources-20261006/README.md)。

下文环境元数据为历史叶；不将旧 APK 或旧实测重绑到上述两个源码提交。

2026-10-06 历史原生 Android 光照环境元数据源码：`8d4fd2ad9920981947256e291b34f0816f62f805`；不是完整光照完成。
抽取原 Windows 四个环境方法/五个 helper，Windows 仅薄委托；原14光照测试及旧测试字节不变。
四公共名字 TimeOfDay/MapInformation/MapChanged/NewMapInfo 经独立 Java observer→原 MainActivity/nativeEvent；
旧个人/实体 callback、认证/会话/宿主旧断言未改。只由已核验 player owner/name/map snapshot 绑定，
16KiB packet/1MiB snapshot；预 owner 单状态聚合，场景清地图保留连接时间，断开清空，准确背压重试。
本叶 render enabled=false、map/entity lights为空；不复制光照 range/palette/material 或战斗/交易/存档规则。
11代码文件、434稳定输入/423受保护整文件，两原 worktree Git checkpoints 保留。
干净精确提交11门502/549、shared1314+10原ignored、runtime296+1原ignored、fresh Java99+99、
API31双check/Mac-host Windows14+21+1+1通过；聚焦计数不相加，不算完整Windows OS验收。
双原生诊断APK/当前variant ELF/6647PNG+3metadata/10原Lighting贴图逐字节对齐已核对，保留数据安装回读SHA一致。
两新原图已查看；仅普通登录隔离及黑底HUD样例（无地图/角色），8秒冷启动PID12571/12638 fatal0。
独立PID GL506=1/0，GPU零错误仍FAIL；没有本叶实际 lighting JNI、灯源或GPU画面验收。
初始编译错误、错误0黑暗值断言、generic observer Debug98/13 FAIL、dedicated observer red5/2均保留；
原62网络断言未改，独立通道修复后双变体99/99。GraphQL EOF/REST备用核验也单列保留。
Gateway为空、preview禁止联网，非WebView；真实登录/玩家闭环/Zone保存/resume/全手机UI/资源音频更新/真机人工仍OPEN。
只读Windows来源仍56ee063fb，较冻结3d735745f +1提交/83文档证据/0功能路径；AP-01–21完整分母不变。
NI-17仍PARTIAL、完整goal Active。下一轮补原地图/对象/效果灯源和实际JNI/GPU证据；仅推Android独立Draft。
[本源APK、十一门、失败、资源与启动边界](generated/player-qa/native-android-lighting-environment-20261006/README.md)。

下文保留对应旧来源的历史记录；不重绑到本叶，也不把旧实测自动计为本叶验收。

2026-10-06 历史 Android 共享 Hero 触控产品源码：`aef89f9426ab7b74d49bf8d61f05eac9f1126c24`。
两个Android文件；429输入/427其他整文件保护，原共享UI/规则/Java/认证/Windows未改。
原生触摸帧位置传给共享Window读者，在Winit Last前恢复；新鲜五门484/531、Java93+93、API31双门通过。
同源双原生诊断APK/当前variant ELF/6647PNG+3metadata已核对，保留数据安装回读SHA一致。
实际六Java/JNI Hero原窗口与普通隔离通过；7张原场景+8张触控图全部查看，四标签切换/两窗关闭/短时不重开通过。
首版068c窗口FAIL、1b点击FAIL和本次未编译采集错误/compiled red0/2→green6/6全部保留。
七冷启动PID仍15条GL506，零错误GPU门FAIL；touch同PID累计日志不相加。手机布局/Hero地图实体/完整操作仍OPEN。
Gateway为空，preview禁止联网，不算真实登录/权威ACK/Zone保存/resume/全资源音频更新/真机人工接受。
只读刷新Windows56ee063fb较冻结3d735745f仍+1提交/83文档证据/0功能路径；AP-01–21完整分母不变。
NI-16仍PARTIAL、完整Windows对齐goal Active。只推Android独立Draft，不部署/改真实存档/推Windows/合并PR。
[本源APK、实际JNI与触控、失败和完整未完门](generated/player-qa/native-android-touch-cursor-20261006/README.md)。

下文保留旧来源的历史记录；其中“当前”仅指当时版本，不重绑到上述源码。

2026-10-06 历史 NI-16 Hero 出站产品源码：`b5b6f4c6893cc62f6f9a9c0a1bf58887cf257605`。
六功能文件；12闭合共享意图、准确pending/actor/epoch、JNI未轮询队列隔离及一行getter生产可见性已落地。
干净精确提交十门478/519、shared1304+10原ignored、runtime296+1原ignored、fresh Java90+90、
API31双检查和Mac-host Windows1+1+21通过；预提交聚焦31，计数重叠不相加。
两个原生诊断APK已保留数据安装到专用API31模拟器，回读SHA/当前variant ELF/选定6647PNG+3metadata匹配。
两张新原图已查看；仅普通登录页隔离及默认离线hud冷启动，PID8803/8866存活/fatal0；
各约6秒样本GL506=0不关闭历史零错误GPU FAIL，不算Hero实际JNI/窗口/操作或真实网络验收。
普通包显示Test server not configured；两个Gateway为空，preview禁止联网。Native strip warning保留，非release体积承诺。
NI-16仍PARTIAL、完整AP-01–21/goal Active；完整手机UI/资源/音频/更新/resume/Zone保存/实际在线/真机人工仍OPEN。
[精确源码、双APK SHA、默认画面、红绿及剩余门槛](generated/player-qa/native-android-hero-egress-20261006/README.md)。

下文为预提交及更早版本的历史快照，不重绑到当前产品源码。

2026-10-06 NI-16 Hero 出站预提交验证快照（基点 `0ebd5c66a705cd4691b92287cb10b5d7ade9126f`）。
12种闭合意图复用原共享Hero pending/epoch/回执，256有界队列/每帧16条；
补齐owner/Hero/场景/焦点/后台屏障，尚未交Java的旧请求准确退役，已交出的未知结果不重放。
六功能文件；第六文件只删除既有只读getter的测试条件，其他入站字节不变。
依据原用户自主实施要求修正此前过严停工判断，不把自动续跑或默认选项当成新增人类批准。
427输入/421受保护整文件/858旧函数/12项冻结wire一致；聚焦31及十门通过：
478/519、shared1304+10原ignored、runtime296+1原ignored、fresh Java90+90、
API31普通/preview均PASS、Mac-host Windows Hero FIFO1/skill FIFO1/协议21（非Windows OS全量）。
旧E0599双FAIL和未轮询队列1通过/3失败保留；仅关闭本批源码/编译门，不等于APK或实际Android JNI。
NI-16仍PARTIAL、完整AP-01–21/goal Active；无本批APK/截图/联网/真机接受，旧GPU FAIL仍保留。
[本批源码接线、红绿及后续精确提交/包证据](generated/player-qa/native-android-hero-egress-20261006/README.md)。

2026-10-06 上一批已提交 NI-16 Hero 入站源码：`0a3b24efa48272b9d1a1b33a63a116b9b5e626f7`。
接通24个公共名字到原共享HeroModel；完整HeroInformation独立512KiB上限，
其余16KiB，角色snapshot绑定后才发布，复用现有skill epoch和物品/tooltip投影。
原到达时钟、原子绑定、准确回执FIFO/背压、切图保留/拒绝Start清空均有回归；
没有复制英雄AI/战斗/物品/技能规则，没有改共享UI、认证、Windows或服务器。
五功能文件；426输入/421受保护整文件、57旧Java测试、10认证方法、140旧Host函数核对。
完整十门448/488、shared1304+10原ignored、runtime296+1原ignored、fresh Java90+90、
API31双门、Mac-host Windows Hero FIFO1/skill FIFO1/协议21通过；不是Windows OS全量门。
两新原生诊断APK保留数据安装并回读SHA一致；选定6647PNG+3metadata匹配。
专用API31 emulator1/physical0，两新截图已看；只复测普通包隔离及既有trade-jni，
不是Hero实际JNI/UI验收。正常PID7409 GL506=1，零错误GPU仍FAIL；
交易模型锁定与UI显示差异保留。Hero入站NI-16为PARTIAL，实际JNI/完整操作、
手机UI重启批准/真实登录与Zone保存/NI17–20/完整资源音频更新/真机人工仍OPEN。
Windows只读来源56ee063fb较冻结3d735745f仍仅+1提交/83文档证据/0功能源；
完整AP-01–21/goal Active保留，不推Windows、不部署或改真实存档。
[本批源码、APK、十门、失败与剩余验收](generated/player-qa/native-android-hero-ingress-20261006/README.md)。

下文只记录对应历史源码/安装包，不重绑到当前本叶。

2026-10-05 历史 NI-15 离线社交 JNI 诊断源码：`3f27e8e9e9a41679eaacebaa3891daf0e5a05b56`。
在单独preview包添加4个精确Java流，经原MainActivity/nativeEvent/owner边界和共享reducer；
正常包不注入，不启用认证、socket或操作/结算。15组员、200行会成员/公告、112仓库槽、
本人125/guest17、u64实例/201与3数量、取消后清空，四场景均实测观测Applied。
修正诊断把TradeAccept误等Trade页的问题：原共享UI实际打开Inventory与两交易窗；
只为离线本人样本补tooltip图标资料，未改共享UI/权限/交易/认证或Windows规则。
四功能文件、69输入在干净提交十门/双APK/安装/JNI前后稳定；65整文件/57旧Java测试/
10认证方法/5冻结helper/8旧preview测试/52旧preview函数不改，旧51场景顺序/唯一性保留。
十门431/471、shared1304+10原ignored、runtime296+1原ignored、fresh Java85+85/API31双门、
Mac-host Windows5+1+21通过；旧compile失败、469场景计数回归、17a交易未观测及红绿保留。
双APK回读安装哈希一致、6647PNG+3metadata匹配，5新截图及同PID离线后台截图已查看；
专用API31 emulator1/physical0。后台采用preview策略，不是正常登录或nativeResume验收。
NI-15仍PARTIAL：模型own_locked=true而ui_trade_locked=false尚未接受，
新PID6336/6590各1条GL0x0506，零错误门仍FAIL；完整手机UI待明确重新批准，
真实登录/在线操作pending/权限邀请报价结算/NI16–20/全资源音频更新/真机与人工仍OPEN。
Windows只读来源56ee063fb较冻结3d735745f仅+1提交/83文档证据/0功能源；
完整AP-01–21与goal Active保留，继续NI16 Hero接线，不推Windows、不部署或改真实存档。
[精确源码、安装APK、JNI画面、失败及剩余门槛](generated/player-qa/native-android-social-jni-20261005/README.md)。

下文只记录对应历史源码/安装包，不重绑到当前本叶。

2026-10-05 历史 NI-15 社交出站源码：`0665a4f4ecc5449ca100d613561f78b40c4a9e76`。
补齐原 AndroidSharedShellPlugin 缺失的 Group/Guild/Trade UI 转发注册；
17种闭合协议字段与冻结Windows相同，沿原共享pending规则捕获准确操作，
只恢复确定未发送的对应草稿/邀请；拒绝旧邀请代次，缺失关联走原断线/DataReset边界。
切图/render-ready/owner/前后台/网络/焦点屏障保留；队列满不覆盖旧项，每帧至多16条。
入队或socket写入不构成权威ACK，不改成员/权限/钱包/物品/锁定/结算规则。
五功能文件仅新增，68输入在干净提交十门/双APK前后稳定；63受保护整文件、
57旧Java断言/10认证会话方法/5冻结物品helper未改，两原worktree Git checkpoint保留。
生产调度注册compiled red0/1→green；最终431/465、shared1304+10原ignored、
runtime296+1原ignored、fresh Java81+81/API31双检查、Mac上Windows5+1+21通过，
不是完整Windows OS gate，也不是实际Android触控/JNI/在线发送验收。
新两APK/ELF与选定6647PNG+3metadata已核对，未安装、无本源新截图；
当前在线设备0模拟器/0真机，历史2be65cc9邮件JNI与GPU FAIL不重绑。
实际社交JNI/原overlay/网络写失败/真实登录权限邀请报价结算/完整手机UI/真机仍OPEN；
NI-15仍PARTIAL、完整AP-01–21/goal Active；UI修补待明确重新批准。
只读刷新Windows来源仍56ee063fb，较冻结3d735745f仅+1提交/83文档证据/0功能源，
SSH ls-remote中断未算成功，以已fetch对象和GitHub只读API一致性核验。
[社交出站精确源码、双APK、十门与剩余验收](generated/player-qa/native-android-social-egress-20261005/README.md)。

下文只记录对应历史源码/安装包，不重绑到当前本叶。

2026-10-03 历史 NI-15 本人报价共享投影源码：`a9698682322d27bab6be8bab69327841f540f89c`。
将冻结 Windows 的 OwnOffer 字段及纯展示函数抽入共享 native_trade_ingress，
Windows 仅薄委托且原5测试/fixture字节未改；Android在既有核验角色snapshot后调用。
服务端自己的nonce/10槽位/u64实例/计数/tooltip/金额/锁定与guest报价保持分离；
缺失/无效/重复/已完成报价不覆盖，显式清空仅清本人；不打开、重开或结算交易。
五功能文件，65输入在干净提交九门/双APK前后稳定；60非本叶整文件/57旧Java断言/
10认证会话方法/5物品helper未改，共享生产函数和字段schema与冻结Windows归一相同。
九门421/454、shared1297+10原ignored、runtime296+1原ignored、fresh Java81+81/
API31双检查、Mac host上Windows原5报价+1guest-tooltip回归通过；不是Windows全量OS gate。
新普通/preview APK及ELF SHA、选定6647 PNG+3metadata已核对，未安装、无本源新截图，
实际social JNI/原overlay消费/出站/真实账号在线操作与结算/手机UI/真机仍OPEN。
NI-15仍PARTIAL、完整AP-01–21/goal Active；UI修补待明确重新批准，旧GPU FAIL保留。
真正Windows来源56ee063fb仍较冻结3d735745f仅多1提交/83文档证据/0功能源，分母不缩减。
两原worktree Git checkpoint保留，不声称未跟踪字节证明。
[本人报价精确源码、双APK、九门和剩余验收](generated/player-qa/native-android-own-trade-20261003/README.md)。

下文只记录历史对应源码/安装包，不重绑到当前本叶。

2026-10-03 历史 NI-15 社交入站源码：`d4dbb124c2a4fe63364571fe48bd5af8b6463bee`。
接通25个公共Group/Guild/Trade名字到原共享SocialModel及native FIFO；
认证后listed-Start暂存，既有角色snapshot绑定才发布；候选原子提交，错角色/
hero/超界拒绝，背压保序，断开/启动拒绝清空、切图保留个人状态。
五Android功能文件；61输入稳定，56非本叶整文件/53旧Java断言/10认证会话方法未改，
5物品补全/数值helper函数体与冻结Windows归一token相同。完整七门412/445、
shared1292+10原ignored、runtime296+1原ignored、fresh Java81+81/API31双检查通过。
编译红日志保留；初版Java红XML复制目录被green重用，原XML未保留，不宣称全量原始
证据无覆盖；最终Java分检查名归档。128字符样本纠正为原32字符，不放宽共享规则。
新普通/preview APK分别533331663/541337559bytes，原生库SHA/选定6647PNG+3metadata
已核对，但未安装、未新截图/JNI/online或真机验收，旧2be65cc9邮件证据不重绑。
自己的trade snapshot/社交出站/实际JNI/在线会员权限邀请报价结算仍OPEN，NI-15
现为PARTIAL；完整AP-01–21/goal Active，UI设计修补仍需人明确重新批准，GPU旧FAIL保留。
发布前重新核验真正Windows来源56ee063fb，较冻结3d735745f仍仅1提交/83文档证据/
0功能源；冻结完整分母不缩减。两原worktree Git checkpoint保留，不声称未跟踪字节证明。
[NI-15精确源码、两APK、失败/七门与剩余验收](generated/player-qa/native-android-social-ingress-20261003/README.md)。

下文只记录对应历史源码/安装包，不重绑到本叶。

2026-10-03 历史 NI-14 实际邮件 JNI 源码：`2be65cc9eb43205a4bdd77c808e367de67206829`。
四种离线 Java→nativeEvent→角色/代次有界邮箱→原共享反馈消费者实际通过；
256封×5附件/上限u64、金777/袋12不被样本回执授予或结算。0b61463e的实际
误断线由0268ad60隔离邮件与Android图形解码修复；其观察器短请求已清空的
FAIL由2be65cc9保存预览观察元数据修复，两次编译红及原始失败包/日志均保留。
累计五功能文件；最终58输入/七门396/429、shared1292+10原ignored、runtime296
+1原ignored、fresh Java77+77/API31双检查通过；11受保护整文件/冻结字段不变。
新源码双APK/选定6647PNG+3metadata/-r保留数据安装SHA绑定；Code仍35。
6本版原图已看，四结果/同PID离线邮箱恢复/普通包隔离通过；小控件/IME/草稿
保留或窗口关闭/实际pending/在线结算未验收。五PID GL506=58/22/50/0/0共130，
GPU零错误仍FAIL，不作跨版性能比较。预览APK540980071字节，ZIP有149188132
未引用到中央目录前的空隙（含签名/填充），仅诊断，不宣称包体优化或正式发布。
真实登录/Zone/保存/完整UI/NI-15–20/资源音频更新/真机人工仍OPEN，NI-14
仍PARTIAL、完整goal Active；UI设计修补仍待人明确确认，冻结Windows分母
3d735745f不缩减。真正Windows来源56ee063fb仍仅多1提交/83文档证据/0功能源。
[三源码失败链、精确APK、四种实际JNI与剩余门](generated/player-qa/native-android-mail-jni-20261003/README.md)。

下文只记录各自历史源码/安装包，不重绑到本叶。

2026-10-03 历史 Android 无损 u64 宿主源码：`f895302be399529df3163cbf3c44499772acd8c2`。
修复默认 JSON 将上限邮件ID转为Double的问题；网络/快照克隆/原生出站采用有界
标准Tokener扩展，正整数u64仍按数字写出，字符串/浮点/溢出拒绝，不改协议或认证规则。
四个Android源/测试文件；53旧输入/11受保护整文件/7认证方法/旧Java断言未变，
connect仅换解码器、MainActivity仅两处解析替换。57输入/七门新鲜394/422、
shared1292+10原ignored、runtime296+1原ignored、Java74+74/API31双检查通过。
新双APK/选定6647PNG+3metadata/-r保留数据安装SHA绑定f895302b；Code仍35，不冒称新版本。
实际API31默认解析红、TLS产品红1/1失败保留；两份安装APK各40项数字探针通过，
探针不夹带产品解码器。此门不等于Activity pump/邮件JNI/真实登录或在线结算。
5原图已看：邮箱同PID恢复/既有商城JNI/正式包预览隔离通过，仅离线；邮箱小控件/
IME裁切FAIL。四独立PID冷日志0/0/23/0，GPU零错误仍FAIL，不据错误次数差宣称优化。
上限u64离线宿主边界已验证，但邮件实际JNI/获准在线流程/Zone/保存/完整UI/
NI-15–20/资源/音频/更新/真机/人工仍OPEN，NI-14仍PARTIAL，完整goal Active。
UI设计修补仍待人明确确认；冻结分母3d735745f不缩减，真正Windows来源56ee063fb
仅多1提交/83文档证据/0功能源，原工作区Git checkpoint保留，独立PR253保持Draft。
[无损ID源码、精确APK/设备探针/失败与完整剩余门](generated/player-qa/native-android-mail-wire-20261003/README.md)。

下文为对应历史源码与实测，不重绑到本叶。

2026-10-03 历史 NI-14 邮件结果接线源码：`4aee63e635ee64befcd9af75c96f769bc13eb61e`。
只有真实认证 WebSocket 写成功才记录自身 send/claim；私有回执绑定本连接代次、
角色和原 claim ID，拒绝伪造/串类/旧连接。ACK 不授予金币物品，等待权威邮箱刷新
才交给原共享反馈消费者；runtime 保留回执并隔帧消费后续邮箱，不改邮件规则或 UI 布局。
五源文件；四组产品红保留并转绿，邻近 Android44/runtime16/Windows邮件5通过。
精确干净提交56输入/七门：Android394/422、shared1292+10原ignored、runtime296
+1原ignored、fresh Java65+65（每变体六类）、API31双check；12受保护整文件、
8认证方法/5原runtime方法/30旧runtime测试规范化一致。
新双原生APK/包内库/选定6647PNG+3metadata/模拟器安装SHA均绑定4aee63e6；
保留数据以-r -d从失败v36回到源码版本35，版本名仍0.1.32-gameshop-phone，不冒称新版本号。
5原图已查看，离线邮箱同PID恢复、既有商城实际Java→JNI与普通包预览隔离通过；
邮箱桌面小控件/IME裁切未通过。四独立PID冷日志GL506=7/24/29/12，共72，GPU零错误仍FAIL。
邮件结果实际JNI、在线收发结算/真实登录/Zone/保存/真机/完整UI/NI-15–20仍OPEN，
Android上限u64 JSON仍缺（超signed-long的claim拒绝而不猜ID），NI-14仍PARTIAL。
UI设计修复仍待用户明确确认；完整goal Active，冻结Windows分母3d735745f不缩减。
本轮真正Windows来源56ee063fb仅多1提交/83文档证据/0功能源，旧分支6ae080711未变。
原两工作区Git checkpoint保留，不宣称未跟踪内容递归哈希；PR253保持独立Draft。
[源码、APK SHA、实测原图、全部失败分类和剩余门](generated/player-qa/native-android-mail-feedback-20261003/README.md)。

下文为各自历史源码/安装包证据，不重绑到本叶。

2026-10-03 历史源码阶段 NI-14 邮件出站：`49693ab1a66ca1ba5573895ce825ea091d5864f4`。
共享 UI 的读信、收取、删除、锁信、邮费询价、附件锁定和发信已进入 Android
现有认证宿主队列；保留盖章选项和原附件 UID/Bag1/Bag2 校验，不复制邮费、资格、
物品或结算规则。冻结 Windows 附件 helper 与七种序列化字段一致，12 个受保护
规则/消费者/Windows/认证文件字节未变；仅增加传输接口，没有 UI 设计修改。
原四项编译红 0/4→4/4；扩展 8/2 属测试夹具缺项，补正式共享清理消费者后 10/10，
断言不删改。精确干净提交 56 输入/七门通过：Android388/416、shared1292+10原ignored、
runtime292+1原ignored、fresh Java59+59（各六类）和 API31 双检查。
C copy/callback/generation 仅 headless；TLS 是本地 JVM 夹具。收取模型读回可复用原
reconcile，但 MailSent/ParcelCollected 的本连接/意图结果关联仍未接通，不虚构 ACK。
本阶段无新 APK/安装/实际 Java→JNI/截图/在线邮件或真机验收；当前只读发现
1 台模拟器、0 台真机。Rust 完整 u64 不等于 Android JSON 全范围实测，仍待核验。
NI-14 仍 PARTIAL、完整 goal Active；NI-15–20、完整 UI/渲染/资源/音频/更新/
认证/Zone/保存/真机/人工验收保持 OPEN。v36 仍回退，UI 修复仍待用户确认继续。
冻结分母 3d735745f 与本轮远端 Windows6ae080711 不变，原工作区保留。
[精确源码、失败分类与剩余门](generated/player-qa/native-android-mail-egress-20261003/README.md)。

下文只读接线和旧包结果均是对应版本的历史记录，不重绑到本阶段。

2026-10-03 历史邮箱只读接线 NI-14：`3003f0780c7254b69ff880a32161cce17142dab2`。
Android 已接通共享邮箱只读列表、服务端邮费和附件锁定事件；256封×5附件、
角色/场景/拒绝启动/背压/溢出测试通过。14个投影函数与冻结Windows一致，
原邮件规则、消费者函数体、认证、Windows宿主及UI设计不变；只封装消费者注册。
精确干净提交53输入/七门通过：Android378/406、shared1287+10原ignored、
runtime292+1原ignored、fresh Java57+57及API31双检查；原始失败和准备错误保留。
本叶未构建/安装新APK或完成实际JNI/联网/收发结算/手机邮箱/真机验收，NI-14仍PARTIAL。
失败v36布局已回退；旧安装包未更新，UI修复待用户确认继续。完整goal Active，
冻结分母3d735745f与远端Windows6ae080711不变。下一叶审计邮箱出站及精确JNI。
[源码、失败与完整未完成边界](generated/player-qa/native-android-mail-ingress-20261003/README.md)。
下文包/模拟器结果仅对应各自历史源码，不重绑到本叶。

> 2026-10-03 最新实际 v35：精确源码 `bc12249ae35c052d6b5c66aa01f4c04a0f95dee6`，
> 已普通快进推送并核验独立 PR253 保持 Draft。五源文件新增可选手机商城呈现，
> 保持105行/14页、原共享动作/目录/购买规则；Windows默认696×476与旧测试保留，
> 认证/Java JNI样本/仓库规则/渲染未改。编译后1/2红→3/3绿，最终手机14/14；
> E0716准备编译失败单独保留。精确提交新鲜359/387、shared1281+10原ignored、
> runtime292+1原ignored、fresh Java52+52/API31双检查通过；50输入/七门/双原生APK/
> 选定资源/保留数据安装SHA绑定。32本版原图查看；控件≥48dp、文字14/16dp，
> 数量2/原共享确认/遮挡/NO/独立Filters滑动、完整14页/末库存3/边界/同PID恢复通过。
> 小横屏单列/末页/下滚控件可达且尺寸已reset，仍有渲染错误；正式包诊断隔离通过。
> 商城/正式两PID GL506=145/12，总157，rbo总158、fatal0；零错误门仍FAIL，不宣称GPU修复。
> 原日志累积快照与环形覆盖已区分，32 PID日志无损gzip/原压缩双SHA；406载荷（含归档准备失败），
> 不改PNG、不把最新丢失JNI标记当作未接通。手机商城IME/实际预览/更窄页脚/竖屏/
> 九语言/真机多指、仓库遮背包/密码IME仍OPEN；本版未做仓库设备回归。
> NI-12/13仍PARTIAL，真实联网/购买/存取/认证/Zone/保存、完整UI/NI-14–20/资源/
> 音频/更新/真机/人工验收仍OPEN，完整goal Active/冻结分母3d735745f不变；
> 原两工作区Git状态未变，未递归哈希未跟踪内容。
> [手机商城呈现、精确APK、实际触控及失败/剩余门](generated/player-qa/native-android-gameshop-phone-20261003/README.md)。
> 下一叶仓库/背包成组呈现、剩余手机输入与完整NI队列，不把本叶替代整套对齐。
> 下文v34及更早仅历史，不覆盖本叶。


> 2026-10-03 最新实际 v34：源码 `76d51415b5ceed667898a720c698b3a062bf7790`，
> 已普通快进推送并核验独立 PR253 保持 Draft。本叶只补共享商城保留窗口遗漏的页码缓存键，
> 不改目录/排序/购买/认证/Java JNI/Windows 宿主/渲染。文字阻挡假设已排除；
> 两项修正夹具后的编译红→七项绿，旧测试未改；准备编译/排序错误原样保留。
> 新鲜359/387、shared1267+10原ignored、runtime292+1原ignored、Java52+52/API31双检查，
> 49输入/七门/双原生APK/选定资源/安装SHA已绑定。21原图逐张查看，实际商城点到1–14页，
> 末库存3/末页边界/返回上一页/同PID恢复通过；仓库II页/恢复和正式包预览隔离复验通过。
> 商城/仓库/正式三个独立PID GL506为6/0/11，总17，零错误门仍FAIL，不宣称渲染改善。
> 手机控件偏小、仓库遮背包仍未完成；数字后缀剥离属原有friendly-name规则，不据此称裁字。
> NI-12/13仍PARTIAL，真实联网/购买/存取/认证/Zone/保存、完整UI/NI-14–20/资源/音频/更新/
> 真机与人工验收仍OPEN，完整goal Active/冻结分母3d735745f不变，原工作区Git状态未变。
> [实际翻页修复、精确APK/原始失败/21原图与剩余门](generated/player-qa/native-android-gameshop-page-cache-20261003/README.md)。
> 下一叶处理可选手机商城呈现与仓库/背包成组布局，复用共享动作与规则；继续完整矩阵。
> 下文v33及更早仅为历史记录，不覆盖当前本叶状态。

> 2026-10-03 最新实际 v33：干净源码`0bfa1632c375a11dd54d6c70783edcff652af59e`。
> 三提交/五个 Android 文件接通诊断 Java→实际 JNI→宿主→共享商城/仓库：
> 105目录/末库存3、160物品/末159槽/扩展/锁定状态得到实际共享回执；不手填模型。
> v31/v32离线角色名不一致触发正常 render身份校验的失败保留；仅修样本身份与独立
> 后置 ResizeStorage 事件，不改正式认证/共享规则/Windows/渲染。最终359/387、
> shared1260+10原ignored、runtime292+1原ignored、fresh Java52+52/API31双检查通过。
> 每版47输入/七门/双原生APK/选定6647PNG+三metadata/保留数据安装SHA绑定。
> v33八张有效原图已查看；仓库实点第二页/同PID恢复保留通过，商城翻页点按/长按FAIL，
> 仓库遮背包与登录/商城手机目标FAIL。锁定模型通过，但密码FLAG_SECURE使三次
> 原截屏为空，未解除保护、不称可见IME通过。四独立PID共52 GL506，零错误门FAIL。
> 正式包传预览参数仍是未配置测试服的登录页；仅离线JNI通过，不算真实HTTPS/WSS/
> 交易/存取/认证/Zone/存档/完整UI/资源/音频/更新/NI-14–20/真机。NI-12/13仍PARTIAL，
> 完整goal Active/冻结分母3d735745f不变；远端Windows仍6ae080711、原工作区Git状态未变。
> [三版精确源码、实际JNI/原图、失败与剩余验收](generated/player-qa/native-android-personal-jni-20261003/README.md)。
> 下一叶定位商城箭头命中并做商城/仓库手机呈现，保持共享控制器；继续完整矩阵。
> 下文NI-13/v31之前仅为各自历史记录，不覆盖当前状态。

> 2026-10-02 最新源码叶 NI-13：`81eb08cdca3390262e1eff3756afd4d09a16d005`。
> 六个源文件接通共享仓库只读投影、Android 角色专属有界 FIFO，以及 Java 公共四包
> 白名单：UserStorage/StorageUnlockResult/StoragePasswordResult/ResizeStorage。
> 十个函数体 token 保留冻结 Windows `3d735745f`；槽位/当前数量图标复用既有
> 共享 helper，Android 仅补包内几何。160 格/稀疏159槽、内容只更新 items、锁定/密码/
> 扩展结果、启动拒绝、角色切换、切场、背压与溢出终止批次的源码夹具通过。
> 两项 Java 编译后红→绿已保留；基线比较与错误变体任务名属于检查准备失败，单独记录。
> 精确提交新鲜359/384、shared1260+10原ignored、runtime292+1原ignored、
> 强制fresh Java47+47（每变体五类）、实际 API31 arm64 双检查通过，七门前后六输入一致。
> 旧仓库规则、精确存取回执通道、原生 critical 队列、Windows/认证/渲染未改；原两工作区
> HEAD/分支/status未变，不宣称未跟踪内容递归哈希。此前商城/证据已核验发布600d8374a，
> PR253仍Draft；本叶待正常发布。完整分母仍冻结3d735745f，不随远端6ae080711回指降级。
> 没有新 APK、实际 Java→JNI/仓库画面/在线存取/密码操作或真机验收；v30旧包不重绑。
> NI-13整体仍PARTIAL、完整goal Active；渲染零错误门仍FAIL，完整UI/NI-12购买Mail/
> NI-14–20/认证/Zone/存档/完整资源/音频/更新/真机仍OPEN。
> [NI-13精确源码、原始失败与未完成门](generated/player-qa/native-android-storage-ingress-20261002/README.md)。
> 下一阶段补 NI-12/13 精确包、实际宿主和模拟器验证，并继续完整矩阵，不以源码叶替代整套对齐。
>
> 下文 NI-12/v30 及更早为各自历史源码和验收，不覆盖本叶。
>

> 2026-10-02 最新源码叶 NI-12：`92033b910cc9ddd996c416ec4024af4625e78b31`。
> 六个源文件接通 Java 公共商城元数据白名单、Android 角色绑定/有界 FIFO、共享
> GameShopInfo/Stock 投影；两投影与四标量 helper 的函数体 token 和冻结 Windows
> `3d735745f`一致，旧商城规则/原生队列/Windows/渲染未改。105行+105库存包夹具、
> 先库存后目录、启动拒绝、身份/切场/背压/终止批次通过；2项编译后 Java 红已保留。
> 最终343/368、shared1257+10原ignored、runtime292+1原ignored、fresh Java44+44、
> 实际 API31 两变体检查通过；错误工作目录的检查准备失败单独保留，不称产品红绿。
> 本叶没有新 APK、模拟器画面、实际 Java→JNI／线上购买／Mail 或真机验收；不重绑
> v30旧包。渲染零错误门仍FAIL，Windows完整分母不变；远端提交前核验仍6ae080711。
> NI-12整体仍PARTIAL；其他完整UI/NI-13–20/认证/Zone/存档/完整资源/音频/更新/真机
> 继续OPEN，完整goal Active。原工作区Git HEAD/分支/status未变，未递归哈希未跟踪内容。
> [NI-12精确源码、原始失败与剩余验收](generated/player-qa/native-android-game-shop-ingress-20261002/README.md)。
> 下一源码叶为NI-13仓库内容/锁定/扩展模型；商城精确包/JNI/实测及此前证据上传仍待完成。
>
> 下文v30及更早仅为各自历史源码与验收，不覆盖本叶。
>
> 2026-10-02 历史实际 v30：精确干净源码`2c9ad6f96ebc47e07056abbf5a6117c278836083`。
> 五个源文件完成可选共享手机 NPC 任务列表的有界重排，复用原控制器/奖励选择，
> 不改认证、任务规则、Windows 宿主或渲染。八行可达，左右独立滑动/上下按钮，
> 18 控件实测≥48dp、文字14/16dp；实际未选奖励提示/OK、选第2/8任务、选奖励B、
> 同PID恢复保留选择/滚动、Leave后恢复仍关闭，26原图已逐张查看/315原始字节证据。
> 编译0/3红、引入的确认控件23/1回归均保留，独立NPC视图缓存修复后24/24绿；
> 新增场景43→44计数断言修正单独记录，不称玩法修复。最终327/352、shared1254
> +10原ignored、强制fresh Java41+41/API31双检查通过；41输入/双原生诊断APK/
> 选定6647PNG+三metadata/实际v29→v30保留数据安装SHA绑定。五PID仍38 GL506，
> 零错误渲染FAIL，不与旧版计数作趋势；两渲染文件仍与277ab相同，依赖范围待答复。
> 完整Windows分母仍3d735745f；本轮观察远端仍6ae080711，不降级/改写Windows。
> 仅NI-10手机NPC呈现叶通过；完整任务动作/路由/Help、compact/IME/九语言/多指、
> 完整NI-11–20/实际认证JNI/HTTPS/WSS/Zone/存档/完整资源/音频/更新/真机仍OPEN。
> 下一叶NI-12权威商城目录/库存模型宿主接线；不把手工UI选择称为真实领取/奖励。
> 完整goal继续Active；原工作区HEAD/分支/status未变，不宣称未跟踪内容递归哈希。
> [精确v30源码、实际NPC触控和剩余未完成门](generated/player-qa/native-android-npc-quests-20261002/README.md)。
>
> 下文v29及更早仅为各自历史源码和验收范围，不覆盖本轮状态。

> 2026-10-02 历史实际 v29：精确干净源码`2161a940ec7a59e966ed2638667f0c7b1a8e32b6`。
> 五个源文件扩展可选共享手机任务确认/提示；复用原控制器，无认证/任务规则/Windows
> 宿主改动。实际No取消、长消息滑动/上下按钮/OK关闭、同PID后台恢复通过有界离线
> 验证，14/16dp、四确认/三提示控件≥48dp；16张原图已查看，259项原始字节证据。
> 3项编译红→3/3绿，新消息旧滚动的最终同初始化场景0/1红→最终共享门绿；
> 测试准备/错误期望/版本核验脚本失败另外保留，不称产品修复。最终327/351、
> shared1247+10原ignored、强制fresh Java41+41/API31双检查通过；41输入/双原生
> 诊断APK/选定6647PNG+三metadata/实际保留数据安装SHA绑定。五PID仍17 GL506，
> 零错误渲染FAIL；两渲染文件与277ab相同，未做GPU修复，依赖扩大范围问题仍待答复。
> 完整Windows分母保持3d735745f；v28记录远端回指6ae080711的异常，不降级/改写。
> NPC任务手机列表、compact/IME/九语言/多指、完整NI-11–20/实际认证JNI/
> HTTPS/WSS/Zone/存档/完整资源/音频/更新/真机仍OPEN，完整goal Active。
> 下一叶NPC任务手机列表及剩余有界宿主接线，不把手工UI样本称服务端任务/奖励。
> [精确v29源码、实际触控和未完成门](generated/player-qa/native-android-quest-modal-20261002/README.md)。
>
> 下文v28及更早仅为各自历史源码和验收范围，不覆盖本轮状态。
>
>

> 2026-10-02 历史实际 v28：精确干净打包源码`84fe8e6344bb0c041e01c66a149b5b42b8198565`。
> 三轮临时GPU探针已全部移除，两个渲染文件与277ab已发布基线逐字节相同；
> **没有游戏渲染修复**。新鲜327/350、shared1238+10原ignored、强制fresh Java
> 41+41、实际arm64 API31双检查通过，41源码输入/双APK/选定6647PNG+三metadata/
> 安装SHA绑定。v28三个独立PID仍24条GL506，零错误渲染FAIL；24张本轮原图已查看。
> 实测排除无效surface存储与活跃ViewTarget串线；私有4×4 FBO两次复现模拟器旧
> 附件类型状态，显式类型清除仅在私有复现恢复，不代表实际游戏修复。约83文件
> 固定依赖/一个Android路径候选已单次询问范围，尚无授权、未实施，其他安全叶继续。
> 完整分母冻结3d735745f；远端Windows现指6ae080711，落后上轮56ee33提交/
> 冻结源32提交，原因未调查，API300文件列表有上限；不降级、不改写Windows。
> NPC任务手机列表/确认/提示、compact/IME/九语言/多指、完整NI-11–20、
> 实际认证JNI/HTTPS/WSS/Zone/存档、完整资源/真机仍OPEN，完整goal Active。
> 保留每版原始失败，不把探针准备编译失败称作产品修复红绿，也不重绑旧APK。
> [精确v28基线、三个假设与未关闭渲染门](generated/player-qa/native-android-gles-investigation-20261002/README.md)。
>
> 下文v24及更早仅为各自历史源码和验收范围，不覆盖本轮状态。

> 2026-10-02 历史实际 v24：精确干净源码`9a6bff0db9a7ff3eed003c72c90310ec632572d0`，
> 四文件可选共享手机任务重排，不复制规则；Windows 默认布局/控制器保留，现来源
> 56ee063fb仅多1提交/83文档/0功能源，冻结3d735745f分母仍有效。任务文字14/16dp、
> 首屏6控件实测≥48dp；实际点行、滑动、上下滚动、关列表留独立详情、最终恢复HUD
> 通过有界验证，13原图已查看/162原证据。编译0/1行尺寸红；5/2触摸失败定位每帧
> 重建，手机视图保留控件后8/8绿。最终327/350、shared1238+10原ignored、fresh Java
> 41+41/API31双检查通过；39输入、双APK/选定资源/安装SHA绑定。五PID仍82 GL506，
> 零错误渲染FAIL；NPC任务列表/确认/提示、九语言、compact/IME/完整手机UI尚缺。
> 初次preview编译失败及历史失败原样保留，不重绑旧包。实际认证JNI/HTTPS/WSS、
> 完整NI-11–20、玩家/Zone/存档/完整资源/真机仍OPEN，goal Active/完整分母不变。
> [精确v24重排、实际滚动与剩余失败](generated/player-qa/native-android-quest-reflow-20261002/README.md)。
>
> 下文v23及更早仅保留各自历史源码和验收范围，不抹除失败。
>

> 2026-10-02 Android exactef5e56/v23 protected quest workspace checkpoint
>
> Only four Android files change; shared renderer/controllers/rules and Windows
> stay unchanged. 38 committed inputs/clean dual diagnostics/selected resources/
> installed hashes pass;325/348/shared1230+10existing ignored/fresh Java41+41/
> actual API31 pass. Compiled occlusion regression0/1 red then1/1 green. Actual
> row/diary Close retains independent detail; diary/pair/detail leave HUD, six
> belt targets, chat and both thumbs visible. NPC Exit restores normal layout.
> Ten original frames inspected/147 raw artifacts; six PIDs38 GL506 still FAIL,
> fonts/48dp/full phone reflow FAIL, compact/IME not accepted. All old failures
> retained. Next readable reflow/scroll, bounded GPU and full NI-11–20. No real
> JNI/login/quest rewards/Zone/save/full resources/device/backend/global pass.
> Whole Windows-alignment goal stays Active; original checkouts preserved.
> [Exact v23 and remaining failures](generated/player-qa/native-android-quest-workspace-20261002/README.md).


> 2026-10-02 Android exactafea4b/v22 shared quest focus checkpoint
>
> Both clean diagnostics/37 inputs/selected resources/installed hashes pass;
>320/343/shared1230+10existing ignored/fresh Java41+41/API31 pass. Shared source
> changes exactly two public markers, all other bytes parent-identical. Production
> Android adapter fits the actual diary/detail union; compiled0/1 red then1/1 green.
> Actual row tap opens both windows with retained gap. Phone UI FAIL: enlarged
> pair obstructs part of HUD/belt/chat and tiny/dark text/targets remain. NPC Exit
> restores chrome. Seven originals inspected; five PIDs67 GL506 retain renderer
> FAIL, prior33/73/135 and old failures unchanged. Next protect shared HUD and
> thumb zones, then readable reflow/48dp/scroll, bounded GPU and full NI-11–20.
> No live JNI/quest/reward/Zone/save/device/backend/global acceptance; goal Active.
> [Exact v22 and retained occlusion/readability failures](generated/player-qa/native-android-quest-focus-20261002/README.md).

> 2026-10-02 exact545a34/v21 feature-only NPC Exit checkpoint
>
> Two-file Android fixture/version repair releases the pinned manual panel;
> open received dialogue still blocks actions. Compiled red0/1 then green1/1;
>315/338/fresh Java41+41/API31 and35 inputs/packages/installed hashes pass.
> Two actual Exit runs restore HUD/actions; one restored Menu tap opens rail.
> Seven originals inspected. Four PIDs33 GL506 retain renderer FAIL; v20 phone
> quest readability FAIL is not repaired/rerun. No authenticated JNI/online Exit,
> full quest/receipt/Zone/save/device acceptance; NI-10 PARTIAL, whole goal Active.
> Next phone quest layout, bounded GPU and complete remaining denominator.
> [Exact v21 and retained failures](generated/player-qa/native-android-npc-exit-20261002/README.md).

> 2026-10-02 exact7b62/v20 received quest/dialogue package leaf
>
> Clean native diagnostics includeccdd ingress;35 input hashes/6647 PNG+three
> metadata each/installed APK hashes pass.315/337/fresh Java41+41/API31 pass.
> Actual offline list/detail(description/turn-in/2/2), dialogue and Exit window
> removal visible in seven original frames. HUD-after-Exit and phone quest
> readability FAIL; five PIDs73 GL506 retain renderer FAIL. No actual authenticated
> JNI, quest/reward/route/server receipt/Zone/save/device acceptance. NI-10 remains
> PARTIAL and whole goal Active. Next display lifetime/phone layout, renderer and
> remaining complete Windows denominator; old failures unchanged.
> [Exact v20 and all retained failures](generated/player-qa/native-android-quest-preview-20261002/README.md).

> 2026-10-02 exactccdd51a90 authoritative quest/dialog source leaf
>
> NI-10 now PARTIAL source: frozen shared projections, authenticated Java public
> metadata allowlist and bounded Android owner/map/history/dialog/exact-ACK host
> lifetime are wired. Twenty-five bodies/fields/dependencies equal Windows3d.
> Final315/334/shared1230+10ignored/runtime292+1ignored/fresh Java41+41/API31 pass.
> Broader Mac Windows bridge98/1 and quest39/12 remain FAILED, equal the retained
> two-affected-file parent comparisons; not full Windows acceptance. Eleven
> committed source hashes bind all gates; original checkouts preserved. No new
> APK/rendered quest/JNI/live/device pass; installedv19 lacks this leaf and135
> GL506 renderer FAIL remains. Next exact package/offline received dialogue then
> remaining full Windows denominator. Whole goal stays Active.
> [Exact source and original failure evidence](generated/player-qa/native-android-quest-ingress-20261002/README.md).

> 2026-10-02 exact78e7/v19 phone-only NPC touch leaf
>
> Both clean native packages,28 input hashes/selected6647 PNG+three metadata
> and install gates pass. Approved Items/7 is actually visible; four shared NPC
> close taps pass with eight original frames/canonical logs. V18 short/600ms
> close failures and compiled reds remain. Android296/preview315/fresh Java39+39/
> actual API31 variants pass after stale target P2 repair. Nine PID captures135
> GL506 errors retain renderer FAIL; no full phone/NI-10/11/JNI/online/device
> increment. Next full quest/dialog ingress and bounded renderer diagnosis.
> Whole Windows-alignment goal stays Active; no rules or Windows source changed.
> [Exact source/package/touch and remaining gates](generated/player-qa/native-android-npc-touch-20261002/README.md).

Following entries retain their original historical source and acceptance scope.

> 2026-10-02 v18 package / offline shared NPC follow-up
>
> Exact native diagnostic source`aa1f2c4ada37ed880b71da5168d0b080583524a3` includes
> the Windows3d735745f refresh and4e35 NPC ingress. Both clean APKs, source/hash,
> selected resource bytes and installs pass. Four actual offline shared NPC
> windows and native map/object/chat smoke are recorded. Source290/preview309,
> fresh Java39+39 and actual API31 variants pass. V17 no-window failure and two
> red regressions remain preserved; Android-only preview map/observer lifetime
> is repaired. This does not close NI-10, full NI-11, NI-19, real JNI/online/phone.
> Zero-error gate FAILS:45 GL506 startup log entries, one nonexistent Buy fixture
> icon, and an unresolved real close tap. Full phone UI/renderer/resources remain
> OPEN. No production environment or human store was used; goal stays Active.
> [Exact proof and next bounded leaf](generated/player-qa/native-android-npc-package-20261002/README.md).

Following checkpoints retain their historical sources and acceptance scope.

2026-10-02 latest NI-11 source leaf: shared Windows3d735745f catalog/service
projection and Android public six-packet ingress/owner-scene/accepted-request
hooks are connected. Failure-first actual host/Java/cross-frame/close paths are
retained; final Android290/preview304/shared1230+10ignored/runtime292+1ignored,
fresh Java39+39, both actual API31 target checks and six focused Windows tests
on Mac pass. Wider Mac NPC filter19 pass/6 geometry/marker/map-fixture failures
is retained, not a broad Windows gate pass. No new APK/JNI/online/device pass.
NI-10 full quest/dialog remains OPEN; NI-11 is PARTIAL, not purchase success.
NI-19 downstream critical/ACK eviction remains OPEN; no lossless claim.
[Exact tests, source hashes and retained failures](generated/player-qa/native-android-npc-ingress-20261002/README.md).

The following checkpoints retain their original frozen source and acceptance scope.

2026-10-01. Frozen Windows source:
`3f5e61533235921369bc13a7760b4a56b0e467e5`.
Execution goal: [Windows completeness parity](ANDROID-WINDOWS-PARITY-GOAL.md).

2026-10-02 source refresh explicitly imports Windows
`3d735745f1117d42a7859e87604a106351dca935` (14 commits after the initial frozen
source). Normal merge preserves Android source; shared1216/10ignored,
runtime291/1ignored, Android276/preview290 and both actual API31 checks pass.
This does not close NI-10/11: Java public NPC packet ingress and accepted-request/
ordered Exit host wiring are still absent. Latest installed v16 does not contain
this refresh. No new APK/online/device or renderer acceptance.
[Exact source and retained failures](generated/player-qa/native-android-windows-refresh-20261002/README.md).

This is a **source-ingress sub-inventory**, not the complete gameplay acceptance
denominator and not a completion percentage. A shared window, an outbound command
serializer or an offline specimen does not prove its authoritative inbound model.
All real-online and physical-device gates below remain OPEN / EXTERNAL.

2026-10-02 latest phone-only v16: clean8cd2e7eae dual APK/hash/install/input binding
passes. Back->draft retap/1500ms hold reopens IME with qw retained; actual e after
reopening updates it to qwe. Failure-first3 then3/3, Android276/preview290/
Java37+37/API31 pass. Startup10 GL506 entries still fail zero-error/stability.
No new ingress/online acceptance. Windows upstream is now3d735745f (14 ahead of
original frozen source), not imported in v16; normal reviewed integration next.
[Exact v16 and retained failures/delta](generated/player-qa/native-android-chat-editor-20261002/README.md).

2026-10-02 phone-only follow-up: exactd009f2402 v15 displays both received chat
rows above the actual keyboard after failure-first regression/repair. Actual Back,
Set/CHAT BOX/Cancel respond, but tapping the draft after dismissal cannot reopen
the keyboard. Six startup GL0x506 entries fail the zero-error log gate. This adds
bounded presentation evidence, not new ingress, online or stability coverage.
[Exact v15 evidence](generated/player-qa/native-android-phone-chat-layout-20261002/README.md).

## Audited boundaries

- Windows `platform-windows/src/gateway.rs` produces typed shared ingress;
  `SkillPacketCursor` binds skill snapshots, owner casts and exact receipts.
- Android Java `GatewaySession.receive` delivers authenticated world snapshots
  and an explicit gameplay-packet allowlist. It does not forward arbitrary JSON.
- Android `world_projection::project` validates owner/map/self identity and
  produces **world, UI player stats, map and entity** messages. A complete original
  world JSON does not populate every shared model: runtime world ingestion updates
  world/interpolation; typed UI domains have separate consumers.
- Android `shared_shell::receive` queues those four messages and maintains scene
  rendering. `live_entity`, scene effects and label adapters consume bounded
  object packets; they do not replace player inventory, skills or service models.
- Android `gateway_bridge::drain_bounded_inbound_into_models` currently handles
  correlated GameShop/storage/change-password receipts via the shared reducer.
  This is receipt coverage, not catalog/storage/mail/social snapshot coverage.

## Model/event leaves to close

Paths in this table are under `apps/game-client`. `PARTIAL` means a source path
exists with bounded evidence; `OPEN` means a required path/equivalence check is
still missing. Neither label is an online acceptance result.

| ID | Shared/Windows seam | Android source status and next acceptance |
| --- | --- | --- |
| NI-01 | `native_ingest::push_native_world_state` | PARTIAL: validated self/map snapshot and native/render receipts exist. Prove real StartGame plus session/character boundary on the exact APK |
| NI-02 | `push_native_ui_read_model` | PARTIAL: same frozen Windows cursor now supplies snapshot/UserInformation vitals, stats, XP, exact u32 weights/known-zero and appearance fields. Owner/Hero/character/partial/reset/terminal regressions pass257/270/shared1203+8ignored/Java36+36/API31/Mac source6. Exact-sourcef04a74399 v12 installs; actual offline StatsI/II numeric values pass, but weight-bar image/start/Launcher failures retained. Buffs/full packet inventory/live/device OPEN. [Source/package/failures](generated/player-qa/native-android-player-ingress-20261001/README.md) |
| NI-03 | `push_native_map_model` / map presentation | PARTIAL: selected center and bounded local pack. Prove server map transfer, render-ready and approved complete resources |
| NI-04 | `push_native_entity_model_set` / object packets | PARTIAL: shared Hero kind and selected actor/item/gold projection. Prove AOI add/remove, incarnation boundaries and all supported actor families |
| NI-05 | `push_native_inventory_model` | PARTIAL: validated self/map feeds the shared inventory/belt/equipment/tooltip projector. Both source geometry seams pass Android244/preview255/API31/Java33+33/13 Gradle controls. Exact-source452398d4c v10 APKs install,6195 PNGs and both metadata byte-match each; actual offline BAG/images and CHAR sword visible with correct digest marker. Phone layout FAILS (BAG occlusion, small CHAR/login); actual JNI/online custody/touch remain OPEN. [Ingress](generated/player-qa/native-android-inventory-ingress-20261001/README.md), [source/package/emulator](generated/player-qa/native-android-item-geometry-20261001/README.md) |
| NI-06 | `push_native_inventory_operation_ack` | PARTIAL: frozen seven Drop/Move/Merge/SplitItem1/Sell/Equip/Remove receipts pass owner/phase/FIFO/backpressure/shared pending regressions. Legacy Move is coordinate-correlated, not an echoed request ID; DeleteItem/SplitItem stay closed. Complete action/unknown-result and actual JNI/online acceptance remain OPEN. [Source evidence](generated/player-qa/native-android-inventory-ingress-20261001/README.md) |
| NI-07 | `push_native_skill_model` / `SkillPacketCursor` | PARTIAL: Android and Windows share the extracted cursor/projector; exact ms, owner metadata/casts, key ACK and character/map/backpressure regressions pass (Android227/preview236/shared1196+8ignored/Java32+32/API31). Exact-source8eb1a7d34 v8 APKs build/install; actual offline SPELLS shows FireBall/F1 but MagIcon/54 is missing repeatedly, so imagery FAILS. Actual JNI/live outbound/input, approved online combat and physical acceptance remain OPEN. [Evidence](generated/player-qa/native-android-skill-ingress-20261001/README.md) |
| NI-08 | `push_native_wallet_patch` | PARTIAL: public Gained/Lose Gold/Credit share Windows helpers after owner bootstrap; missing base errors, latest absolute UI/wallet retry preserves runtime ordering without delta replay. Numeric/foreign/Hero/character/map/reset regressions pass; exact v12 offline BAG/HUD gold12352 visible. Credit503 is a source marker, not a rendered credit control; Java-to-JNI live server wallet/action/settlement/device remain OPEN, not purchase success. [Evidence](generated/player-qa/native-android-player-ingress-20261001/README.md) |
| NI-09 | `push_native_chat_line` | PARTIAL: frozen shared projector/owner-bootstrap/bounded FIFO; prior v14 clipped-row and v15 reopen FAIL retained. Clean8cd2e7eae v16 dual APK/hash/resource/install/input binding; failure-first3 then3/3, Android276/preview290/Java37+37/API31 pass. Actual soft keys qw, Back->retap/1500ms hold reopen preserving draft, then e updates qwe. Startup10 GL506 entries/zero-error FAIL; extreme IME controls and real Java-to-JNI online outbound+echo/filter/scroll/full settings/device OPEN. Shared runtime critical/ACK eviction unchanged, not lossless acceptance. [Exact v16, failures and upstream delta](generated/player-qa/native-android-chat-editor-20261002/README.md) |
| NI-10 | Quest/NPC gameplay bridge | PARTIAL: frozen tracker/detail/rewards/history/dialog/nearby and exact-ACK lifecycle from ccdd51a90 are included in v30. Historical twenty-five-body equivalence and wider Mac failures remain bound to their original inputs. V24 diary/detail reflow and v29 confirmation/alert are retained. Current source2c9ad6f96 v30 adds optional shared phone NPC list, eight reachable rows, independent list/message scroll, 14/16dp text and 18 controls≥48dp; 327/352/shared1254+10existing ignored/fresh Java41+41/API31 gates pass. Actual unselected Finish/alert/OK, quest2/8 selection, both viewport directions, reward B selection, same-PID resume and Leave stay-closed pass only offline; 26 original frames inspected. Separate received offline diary baseline is not authenticated receipt, and no reward grant is accepted. Compiled0/3 red, introduced23/1 modal regression and inventory-count maintenance failure are retained; final phone24/24. Five PIDs38 GL506 keep renderer FAIL. Full actions/routes/Help/localization/compact/IME/multitouch/authenticated JNI/online rewards/device remain OPEN. [Ingress](generated/player-qa/native-android-quest-ingress-20261002/README.md), [v20 failures](generated/player-qa/native-android-quest-preview-20261002/README.md), [v21 Exit](generated/player-qa/native-android-npc-exit-20261002/README.md), [v24 reflow](generated/player-qa/native-android-quest-reflow-20261002/README.md), [v29 modal scope](generated/player-qa/native-android-quest-modal-20261002/README.md), [v30 NPC scope](generated/player-qa/native-android-npc-quests-20261002/README.md) |
| NI-11 | `push_native_shop_model` / `push_native_npc_shop_service` | PARTIAL: frozen3d735745f shared projection/Java six-packet owner+scene gate/Android FIFO are wired and included unchanged in v30. Twelve extracted bodies, cross-frame pending/order/Exit/CloseWindows/map/terminal/bounds source gates and six focused Mac Windows checks passed in their historical source leaf; wider19/6 failures remain, not a full Windows gate. V19 installed diagnostic evidence includes a visible approved item and actual Buy/Sell/Repair/SRepair close taps; v29/v30 did not retest those service taps. No actual authenticated JNI, online purchase/sale/repair complete receipts, physical-device or complete phone service acceptance. [Ingress/source](generated/player-qa/native-android-npc-ingress-20261002/README.md), [v19 bounded taps](generated/player-qa/native-android-npc-touch-20261002/README.md), [v30 package/current limits](generated/player-qa/native-android-npc-quests-20261002/README.md) |
| NI-12 | `push_native_game_shop_info` / stock / receipt | PARTIAL: source92033b910 adds exact frozen Windows public catalog/stock projections, selected-Start Java whitelist and bounded Android owner/FIFO producer. Six function bodies preserve token equivalence; 105-row+105-stock, stock-before-info, owner/scene/reset/terminal/backpressure fixtures and shared runtime catalog/receipt gates pass. Android343/368/shared1257+10existing ignored/runtime292+1existing ignored/fresh Java44+44/API31 pass; original two compiled Java failures and separate check-preparation failure retained. Legacy purchase rules and native critical/receipt queues unchanged. No new APK/UI/JNI/live catalog/price/image parity, purchase/Credit/Mail settlement or physical acceptance; do not relabel the installed v30 package. Full NI-12 stays PARTIAL. [Source evidence and open gates](generated/player-qa/native-android-game-shop-ingress-20261002/README.md) |
| NI-13 | `push_native_storage_model` / items / patch | PARTIAL: source81eb08cdc adds ten exact frozen Windows pure bodies plus existing shared slot/current-count metadata delegation, four-packet Java public whitelist and bounded owner/FIFO Storage producer. 160/sparse slots, model-versus-items, password/expansion results, rejected Start, owner/map/reset/terminal/backpressure and overflow fixtures pass. Android359/384/shared1260+10existing ignored/runtime292+1existing ignored/fresh Java47+47/API31 pass; compiled two-test red and separate preparation failures retained. StoreItemV2/TakeBackItemV2 stay on the existing correlated receipt channel; old storage rules/native critical queue/runtime/auth/Windows/renderer unchanged. No new APK/actual JNI/UI/live transfer-password-expansion/reconnect-save/physical acceptance; installed v30 cannot be relabeled. Full NI-13 stays PARTIAL. [Exact source evidence and remaining gates](generated/player-qa/native-android-storage-ingress-20261002/README.md) |
| NI-14 | `push_native_mail_model` / `push_native_mail_service` | PARTIAL: source2be65cc9 passes all four actual offline Java→nativeEvent→owner/epoch-bounded mail ingress→unchanged shared feedback/UI consumer cases, including256 mails×5 attachments, fullu64 IDs, gold777/bag12 unchanged and newer refresh. Source0b61463e's actual Android render-decoder disconnect is fixed by0268ad60; its retired-PreviewRequest observer failure is fixed by2be65cc9, with compiled reds and all raw failed APKs/logs retained. Five cumulative functional files; clean58-input/seven-gate results396/429, shared1292+10existing ignored, runtime296+1existing ignored, fresh Java77+77/API31 checks. Exact double APK/resource/data-preserving install hashes; Code35 unchanged. Six current originals viewed, same-PID offline inbox resume and normal APK isolation pass. Five cold PIDs130 GL506 keep GPU gate FAIL; phone targets/IME, actual draft/reader closure, pending operations, real auth/network/settlement remain unaccepted. Preview540980071bytes has a large unreferenced ZIP range; no release-size claim. Full UI/NI-15–20/resource/audio/update/device/human remain OPEN; UI design repair awaits human confirmation and whole Windows-alignment goal stays Active. [Current three-source JNI evidence](generated/player-qa/native-android-mail-jni-20261003/README.md); [historical unsigned-wire evidence](generated/player-qa/native-android-mail-wire-20261003/README.md) |
| NI-15 | `push_native_social_model` | PARTIAL: source3f27e8e9e retains previous25 ingress/own-offer and17 shared outbound types. Four preview-only Java→nativeEvent scenes now observe actual owner-bound shared group15, guild200 members/200 notices/112 slots, exact own125 vs guest17/u64 stacks201+3 and authoritative cancel/unchanged wallet777; normal build isolation and five selected screenshots checked. Original shared Inventory+two-trade-window behavior preserved; only offline item metadata/diagnostic predicate corrected. 69 stable inputs/65 protected whole files, clean ten gates431/471,shared1304+10 ignored,runtime296+1 ignored,Java85+85/API31x2/Mac Windows5+1+21; exact-source dual APKs installed and pulled hashes match. Historical failures kept. Locked UI presentation mismatch OPEN; new PID6336/6590 each GL0x0506, zero-error GPU gate FAIL. Offline same-PID background is not normal authentication/resume. Full UI reapproval pending; actual buttons/pump/socket failures/online permissions/invitations/offers/settlement/reconnect, physical and NI16–20/AP01–21 OPEN; goal Active. [Installed offline JNI evidence and open gates](generated/player-qa/native-android-social-jni-20261005/README.md) |
| NI-16 | `push_native_hero_model` / Hero receipts | PARTIAL: current sourceaef89f942 retains earlier 24 ingress/12 shared outbound contracts, owner/epoch/pending/FIFO safeguards; no shared Hero/UI/Java/auth/Windows rules changed in this touch leaf. Two Android files;429 stable inputs/427 protected whole files. Exact-source five gates484/531,fresh Java93+93/API31x2; no new shared/runtime/full Windows OS rerun. Both diagnostic APKs preserve-data installed, pulled hashes/current-variant ELF/selected6647PNG+3metadata match. Six actual preview Java/nativeEvent owner-bound Hero models/original windows plus normal isolation observed;7 original scene and8 actual touch PNGs reviewed. Four tab transitions,two closes and bounded3s closed wait pass; never count these as online actions. First068c window FAIL,1b actual tap/180ms FAIL and new harness/compiled red0/2 retained. Seven new cold-start PIDs total15 GL0x0506; zero-error GPU FAIL, same-PID touch logs not summed. Phone layout/Hero map entity/full operations/actual socket send/server ACK/real login/Zone-save/nativeResume/resources/audio/update/physical/human/AP01–21 remain OPEN; goal Active. [Bounded native JNI and actual shared touch evidence](generated/player-qa/native-android-touch-cursor-20261006/README.md) |
| NI-17 | `push_native_lighting_render_state` and effects | PARTIAL: current source1f01c1351 includes shared sourcec57a3cf06. Original Windows producer/map hook is shared with thin delegates and byte-unchanged14/4/2 old tests; real pack cells/optional effect metadata, matching asset request/render-ready/committed pose, bounded generation/reset/retry and same-frame consumer scheduling are wired. First actual night/dawn/forced-dark GPU runs exited on unsupported VIEW_FORMATS while PID stayed alive; two-file capability fix preserves the original sRGB render/sample view and supported-device descriptor, without shader/palette/rule changes. Final445-input clean fourteen gates529/577,shared1324+10 existing ignored,runtime301+1 existing ignored,fresh Java103+103,API31x2,Mac Windows14+4+2+21+1+1 and actual-resource gate pass, not full Windows OS acceptance. Dual diagnostic APK/variant ELF/selected6647PNG+3metadata+10 original Lighting frames match, preserve-data install/pulled SHA match. Five new8s cold PIDs13707/13787/13865/13954/14032 stay foreground without render-validation AppExit; four actual offline Java/nativeEvent world-light scenes and normal login isolation observed, all12 original PNGs including7 first-source failures reviewed. Consumer reports6 map/3 entity sources,9 layers/10 loaded original textures in dark scenes; daylight retains0 layers. Producer/consumer markers are not enqueue ACK/GPU fences/exact Windows pixel parity. New five PIDs total8 GL0x0506 retain zero-error GPU FAIL. First13-test red7/6, new-oracle12/1 and descriptor2/1 red retained; original catalogue61->65 exact append is explicit, not weakened. Full light-action variants/map/object coverage, audio/focus/nativeResume, real auth/network/Zone-save, full phone UI,2969 whole-map missing references/full resources-update, physical/human/AP01–21 remain OPEN; goal Active. [Current exact-source packages, failures and bounded JNI/GPU images](generated/player-qa/native-android-lighting-sources-20261006/README.md) |
| NI-18 | native resume negotiation | OPEN: source697d6b545 connects only the implemented `nativeGameShopReceiptV1` negotiation on the actual Java socket, once after clientVersion per connection; helper no longer claims unwired `nativeResumeV1`, and gameplay cannot inject capability/resume controls. Compiled Java red0/5→green5/5 and Rust0/1→1/1, clean fourteen applicable gates, exact dual APK/install/resource and two original startup images are documented above. This does not implement native resume: endpoint-bound in-memory credential ownership, expiry/generation/rejection/cancellation/deadline, background/network retry and native post-resume data/render-ready boundaries remain OPEN. No actual Gateway login/receipt JNI/online resume/save or physical acceptance; zero-error GPU still FAIL and full AP01–21 goal Active |
| NI-19 | data/scene reset and queue backpressure | PARTIAL: existing native resets and bounded JNI/receipt queues. Test every new domain through reconnect, logout, character switch, scene reset, stale receipt and overflow |
| NI-20 | native atlas/version/cache delivery | PARTIAL: frozen UI_32bit470..473 sparse guard20/item13/magic7/Java36+36 pass. Exactcfac6ecd4 v13 APKs install,6647 PNG+3metadata each match; four originals match frozen Git; actual offline BAG471 restored. Phone HUD weight/other ratios/full aligned release still OPEN. Host crash/first timeout retained, not stability acceptance. [Evidence](generated/player-qa/native-android-weight-bars-20261001/README.md) |

## Input/presentation leaves of the integration stage

- GI-01: ordinary Inventory/Character/Skill/Options/Menu/QuestLog retain phone
  status/chat and a dedicated joystick. Source regressions cover both an already
  held joystick and a fresh gesture. BigMap/search and other Windows-supported
  windows still need their own Android layout/input audit.
- GI-02: changing a view cancels the old shared pointer, waits for finger release
  and does not fabricate a click through the new view. True NPC services,
  confirmation notices, quest prompts, skill assignment, chat editing and death
  retain action guards; death keeps the separate Revive affordance.
- GI-03: Android retains physical/OS-logical phone bounds; desktop resolution
  settings/plugin are not installed. Hint geometry tests use density1/2.75/3.5
  and UI-scale0.35/0.5/0.9. These are geometry tests, not actual device DPI proof.
- GI-04: inherited desktop audio visual tests must compile under native-player-ui
  without enabling native-ui/audio. Existing offline visual ignores stay ignored.
- GI-05: PASS for the bounded package gate. New API31 arm64 v7 Debug/uiPreview
  APKs, versions/hashes and actual emulator frames bind to source10bf437f2, which
  includes normal merge b7caac732 and frozenWindows3f5e61533. v5/v6 failure
  artifacts remain distinct. [Exact evidence](generated/player-qa/native-android-windows-g1-20261001/README.md).
- GI-06: v6 actual emulator input exposed movement from the **hidden** joystick
  while the Android phone rail was expanded. A failing regression reproduces
  this for a fresh touch; the subsequent phone-only fix blocks fresh/held motion
  and includes rail changes in pointer ownership. Ordinary shared windows remain
  nonmodal. v7 full Rust tests219/preview227 pass; actual menu-hidden swipe yields
  no new moves, while the open shared bag retains two offline move intents.
  This is not online authoritative movement or multi-finger/device acceptance.

## Implementation order

The v8 missing skill images have a bounded package-source repair: both original
224-frame magic-icon libraries are required and staged, from a new pixel-checked
diagnostic pack. Unique index/path/coverage checks close the reproduced count-only
P2; seven real Gradle controls pass with unchanged source bytes.
Preview237/Java32+32 and negative old-pack rejection pass; exact
v9 source394307db8 builds/installs both diagnostic variants with version/hash
binding and448 byte-matched icons each. Actual offline SPELLS now shows the
original icon/Lv1/F1 without captured asset errors. Both start waits timed out;
phone-window, performance, real JNI/online and device gates remain open.
[Source/resource/package evidence](generated/player-qa/native-android-skill-icons-20261001/README.md).

Bounded G1 source/package/emulator gates and NI-07 source regressions are recorded.
NI-07 is bound to v8 source, with bounded v9 image repair; v7 lacks this implementation.
Continue the G2 real login/list/StartGame gate when the user has
approved a test environment and can enter credentials locally. NI-05/06 now have
a bounded typed-model/seven-receipt source path: Android236/preview246/shared1203
plus8ignored/Java33+33/API31 pass. Correct Mac desktop inventory filter retains
6pass/2 resource-dependent failures; no blanket desktop green. Both
original item geometry source seams are now connected: Android244/preview255 and
both real API31 feature checks/Java33+33 pass; staged source bytes and13 Gradle
controls are verified. Exact-source452398d4c v10 builds/install/6195 frames and
runtime metadata digests are verified; offline BAG and CHAR sword appear. Normal
start wait timeout and phone occlusion/small-window failures remain. Next bounded
phone panel layout/input, then NI-10/11 services; no online or device acceptance.
The remaining model, window, resource and physical-device leaves stay in the goal;
this sub-inventory does not shrink its scope or mark the whole goal complete.
