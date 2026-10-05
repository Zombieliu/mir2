# Android native UI coverage — 2026-09-08

2026-10-06 当前 NI-16 Hero 入站源码：`0a3b24efa48272b9d1a1b33a63a116b9b5e626f7`。
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

2026-10-03 最新实际 v35：精确源码 `bc12249ae35c052d6b5c66aa01f4c04a0f95dee6`，
已普通快进推送并核验独立 PR253 保持 Draft。五源文件新增可选手机商城呈现，
保持105行/14页、原共享动作/目录/购买规则；Windows默认696×476与旧测试保留，
认证/Java JNI样本/仓库规则/渲染未改。编译后1/2红→3/3绿，最终手机14/14；
E0716准备编译失败单独保留。精确提交新鲜359/387、shared1281+10原ignored、
runtime292+1原ignored、fresh Java52+52/API31双检查通过；50输入/七门/双原生APK/
选定资源/保留数据安装SHA绑定。32本版原图查看；控件≥48dp、文字14/16dp，
数量2/原共享确认/遮挡/NO/独立Filters滑动、完整14页/末库存3/边界/同PID恢复通过。
小横屏单列/末页/下滚控件可达且尺寸已reset，仍有渲染错误；正式包诊断隔离通过。
商城/正式两PID GL506=145/12，总157，rbo总158、fatal0；零错误门仍FAIL，不宣称GPU修复。
原日志累积快照与环形覆盖已区分，32 PID日志无损gzip/原压缩双SHA；406载荷（含归档准备失败），
不改PNG、不把最新丢失JNI标记当作未接通。手机商城IME/实际预览/更窄页脚/竖屏/
九语言/真机多指、仓库遮背包/密码IME仍OPEN；本版未做仓库设备回归。
NI-12/13仍PARTIAL，真实联网/购买/存取/认证/Zone/保存、完整UI/NI-14–20/资源/
音频/更新/真机/人工验收仍OPEN，完整goal Active/冻结分母3d735745f不变；
原两工作区Git状态未变，未递归哈希未跟踪内容。
[手机商城呈现、精确APK、实际触控及失败/剩余门](generated/player-qa/native-android-gameshop-phone-20261003/README.md)。
下一叶仓库/背包成组呈现、剩余手机输入与完整NI队列，不把本叶替代整套对齐。
下文v34及更早仅历史，不覆盖本叶。


2026-10-03 最新实际 v34：源码 `76d51415b5ceed667898a720c698b3a062bf7790`，
已普通快进推送并核验独立 PR253 保持 Draft。本叶只补共享商城保留窗口遗漏的页码缓存键，
不改目录/排序/购买/认证/Java JNI/Windows 宿主/渲染。文字阻挡假设已排除；
两项修正夹具后的编译红→七项绿，旧测试未改；准备编译/排序错误原样保留。
新鲜359/387、shared1267+10原ignored、runtime292+1原ignored、Java52+52/API31双检查，
49输入/七门/双原生APK/选定资源/安装SHA已绑定。21原图逐张查看，实际商城点到1–14页，
末库存3/末页边界/返回上一页/同PID恢复通过；仓库II页/恢复和正式包预览隔离复验通过。
商城/仓库/正式三个独立PID GL506为6/0/11，总17，零错误门仍FAIL，不宣称渲染改善。
手机控件偏小、仓库遮背包仍未完成；数字后缀剥离属原有friendly-name规则，不据此称裁字。
NI-12/13仍PARTIAL，真实联网/购买/存取/认证/Zone/保存、完整UI/NI-14–20/资源/音频/更新/
真机与人工验收仍OPEN，完整goal Active/冻结分母3d735745f不变，原工作区Git状态未变。
[实际翻页修复、精确APK/原始失败/21原图与剩余门](generated/player-qa/native-android-gameshop-page-cache-20261003/README.md)。
下一叶处理可选手机商城呈现与仓库/背包成组布局，复用共享动作与规则；继续完整矩阵。
下文v33及更早仅为历史记录，不覆盖当前本叶状态。

2026-10-03 最新实际 v33：干净源码`0bfa1632c375a11dd54d6c70783edcff652af59e`。
三提交/五个 Android 文件接通诊断 Java→实际 JNI→宿主→共享商城/仓库：
105目录/末库存3、160物品/末159槽/扩展/锁定状态得到实际共享回执；不手填模型。
v31/v32离线角色名不一致触发正常 render身份校验的失败保留；仅修样本身份与独立
后置 ResizeStorage 事件，不改正式认证/共享规则/Windows/渲染。最终359/387、
shared1260+10原ignored、runtime292+1原ignored、fresh Java52+52/API31双检查通过。
每版47输入/七门/双原生APK/选定6647PNG+三metadata/保留数据安装SHA绑定。
v33八张有效原图已查看；仓库实点第二页/同PID恢复保留通过，商城翻页点按/长按FAIL，
仓库遮背包与登录/商城手机目标FAIL。锁定模型通过，但密码FLAG_SECURE使三次
原截屏为空，未解除保护、不称可见IME通过。四独立PID共52 GL506，零错误门FAIL。
正式包传预览参数仍是未配置测试服的登录页；仅离线JNI通过，不算真实HTTPS/WSS/
交易/存取/认证/Zone/存档/完整UI/资源/音频/更新/NI-14–20/真机。NI-12/13仍PARTIAL，
完整goal Active/冻结分母3d735745f不变；远端Windows仍6ae080711、原工作区Git状态未变。
[三版精确源码、实际JNI/原图、失败与剩余验收](generated/player-qa/native-android-personal-jni-20261003/README.md)。
下一叶定位商城箭头命中并做商城/仓库手机呈现，保持共享控制器；继续完整矩阵。
下文NI-13/v31之前仅为各自历史记录，不覆盖当前状态。

2026-10-02 最新源码叶 NI-13：`81eb08cdca3390262e1eff3756afd4d09a16d005`。
六个源文件接通共享仓库只读投影、Android 角色专属有界 FIFO，以及 Java 公共四包
白名单：UserStorage/StorageUnlockResult/StoragePasswordResult/ResizeStorage。
十个函数体 token 保留冻结 Windows `3d735745f`；槽位/当前数量图标复用既有
共享 helper，Android 仅补包内几何。160 格/稀疏159槽、内容只更新 items、锁定/密码/
扩展结果、启动拒绝、角色切换、切场、背压与溢出终止批次的源码夹具通过。
两项 Java 编译后红→绿已保留；基线比较与错误变体任务名属于检查准备失败，单独记录。
精确提交新鲜359/384、shared1260+10原ignored、runtime292+1原ignored、
强制fresh Java47+47（每变体五类）、实际 API31 arm64 双检查通过，七门前后六输入一致。
旧仓库规则、精确存取回执通道、原生 critical 队列、Windows/认证/渲染未改；原两工作区
HEAD/分支/status未变，不宣称未跟踪内容递归哈希。此前商城/证据已核验发布600d8374a，
PR253仍Draft；本叶待正常发布。完整分母仍冻结3d735745f，不随远端6ae080711回指降级。
没有新 APK、实际 Java→JNI/仓库画面/在线存取/密码操作或真机验收；v30旧包不重绑。
NI-13整体仍PARTIAL、完整goal Active；渲染零错误门仍FAIL，完整UI/NI-12购买Mail/
NI-14–20/认证/Zone/存档/完整资源/音频/更新/真机仍OPEN。
[NI-13精确源码、原始失败与未完成门](generated/player-qa/native-android-storage-ingress-20261002/README.md)。
下一阶段补 NI-12/13 精确包、实际宿主和模拟器验证，并继续完整矩阵，不以源码叶替代整套对齐。

下文 NI-12/v30 及更早为各自历史源码和验收，不覆盖本叶。

2026-10-02 最新源码叶 NI-12：`92033b910cc9ddd996c416ec4024af4625e78b31`。
六个源文件接通 Java 公共商城元数据白名单、Android 角色绑定/有界 FIFO、共享
GameShopInfo/Stock 投影；两投影与四标量 helper 的函数体 token 和冻结 Windows
`3d735745f`一致，旧商城规则/原生队列/Windows/渲染未改。105行+105库存包夹具、
先库存后目录、启动拒绝、身份/切场/背压/终止批次通过；2项编译后 Java 红已保留。
最终343/368、shared1257+10原ignored、runtime292+1原ignored、fresh Java44+44、
实际 API31 两变体检查通过；错误工作目录的检查准备失败单独保留，不称产品红绿。
本叶没有新 APK、模拟器画面、实际 Java→JNI／线上购买／Mail 或真机验收；不重绑
v30旧包。渲染零错误门仍FAIL，Windows完整分母不变；远端提交前核验仍6ae080711。
NI-12整体仍PARTIAL；其他完整UI/NI-13–20/认证/Zone/存档/完整资源/音频/更新/真机
继续OPEN，完整goal Active。原工作区Git HEAD/分支/status未变，未递归哈希未跟踪内容。
[NI-12精确源码、原始失败与剩余验收](generated/player-qa/native-android-game-shop-ingress-20261002/README.md)。
下一源码叶为NI-13仓库内容/锁定/扩展模型；商城精确包/JNI/实测及此前证据上传仍待完成。

下文v30及更早仅为各自历史源码与验收，不覆盖本叶。

2026-10-02 历史实际 v30：精确干净源码`2c9ad6f96ebc47e07056abbf5a6117c278836083`。
五个源文件完成可选共享手机 NPC 任务列表的有界重排，复用原控制器/奖励选择，
不改认证、任务规则、Windows 宿主或渲染。八行可达，左右独立滑动/上下按钮，
18 控件实测≥48dp、文字14/16dp；实际未选奖励提示/OK、选第2/8任务、选奖励B、
同PID恢复保留选择/滚动、Leave后恢复仍关闭，26原图已逐张查看/315原始字节证据。
编译0/3红、引入的确认控件23/1回归均保留，独立NPC视图缓存修复后24/24绿；
新增场景43→44计数断言修正单独记录，不称玩法修复。最终327/352、shared1254
+10原ignored、强制fresh Java41+41/API31双检查通过；41输入/双原生诊断APK/
选定6647PNG+三metadata/实际v29→v30保留数据安装SHA绑定。五PID仍38 GL506，
零错误渲染FAIL，不与旧版计数作趋势；两渲染文件仍与277ab相同，依赖范围待答复。
完整Windows分母仍3d735745f；本轮观察远端仍6ae080711，不降级/改写Windows。
仅NI-10手机NPC呈现叶通过；完整任务动作/路由/Help、compact/IME/九语言/多指、
完整NI-11–20/实际认证JNI/HTTPS/WSS/Zone/存档/完整资源/音频/更新/真机仍OPEN。
下一叶NI-12权威商城目录/库存模型宿主接线；不把手工UI选择称为真实领取/奖励。
完整goal继续Active；原工作区HEAD/分支/status未变，不宣称未跟踪内容递归哈希。
[精确v30源码、实际NPC触控和剩余未完成门](generated/player-qa/native-android-npc-quests-20261002/README.md)。

下文v29及更早仅为各自历史源码和验收范围，不覆盖本轮状态。

2026-10-02 历史实际 v29：精确干净源码`2161a940ec7a59e966ed2638667f0c7b1a8e32b6`。
五个源文件扩展可选共享手机任务确认/提示；复用原控制器，无认证/任务规则/Windows
宿主改动。实际No取消、长消息滑动/上下按钮/OK关闭、同PID后台恢复通过有界离线
验证，14/16dp、四确认/三提示控件≥48dp；16张原图已查看，259项原始字节证据。
3项编译红→3/3绿，新消息旧滚动的最终同初始化场景0/1红→最终共享门绿；
测试准备/错误期望/版本核验脚本失败另外保留，不称产品修复。最终327/351、
shared1247+10原ignored、强制fresh Java41+41/API31双检查通过；41输入/双原生
诊断APK/选定6647PNG+三metadata/实际保留数据安装SHA绑定。五PID仍17 GL506，
零错误渲染FAIL；两渲染文件与277ab相同，未做GPU修复，依赖扩大范围问题仍待答复。
完整Windows分母保持3d735745f；v28记录远端回指6ae080711的异常，不降级/改写。
NPC任务手机列表、compact/IME/九语言/多指、完整NI-11–20/实际认证JNI/
HTTPS/WSS/Zone/存档/完整资源/音频/更新/真机仍OPEN，完整goal Active。
下一叶NPC任务手机列表及剩余有界宿主接线，不把手工UI样本称服务端任务/奖励。
[精确v29源码、实际触控和未完成门](generated/player-qa/native-android-quest-modal-20261002/README.md)。

下文v28及更早仅为各自历史源码和验收范围，不覆盖本轮状态。

2026-10-02 历史实际 v28：精确干净打包源码`84fe8e6344bb0c041e01c66a149b5b42b8198565`。
三轮临时GPU探针已全部移除，两个渲染文件与277ab已发布基线逐字节相同；
**没有游戏渲染修复**。新鲜327/350、shared1238+10原ignored、强制fresh Java
41+41、实际arm64 API31双检查通过，41源码输入/双APK/选定6647PNG+三metadata/
安装SHA绑定。v28三个独立PID仍24条GL506，零错误渲染FAIL；24张本轮原图已查看。
实测排除无效surface存储与活跃ViewTarget串线；私有4×4 FBO两次复现模拟器旧
附件类型状态，显式类型清除仅在私有复现恢复，不代表实际游戏修复。约83文件
固定依赖/一个Android路径候选已单次询问范围，尚无授权、未实施，其他安全叶继续。
完整分母冻结3d735745f；远端Windows现指6ae080711，落后上轮56ee33提交/
冻结源32提交，原因未调查，API300文件列表有上限；不降级、不改写Windows。
NPC任务手机列表/确认/提示、compact/IME/九语言/多指、完整NI-11–20、
实际认证JNI/HTTPS/WSS/Zone/存档、完整资源/真机仍OPEN，完整goal Active。
保留每版原始失败，不把探针准备编译失败称作产品修复红绿，也不重绑旧APK。
[精确v28基线、三个假设与未关闭渲染门](generated/player-qa/native-android-gles-investigation-20261002/README.md)。

下文v24及更早仅为各自历史源码和验收范围，不覆盖本轮状态。

> 2026-10-01 independent Android/shared sync: merge `007df76c9` imports latest
> verified Windows continuation `4b73525f3` without changing the original dirty
> checkout or Android branch. Shared NPC/mail/chat semantics, IME editor epochs,
> 500 UTF-16 full-document mail editing and process-lifetime EntityAtlas are
> integrated; actual two-line mail input exposed and repaired Android-only
> Winit/Java keyboard ownership. Android affine clipping now fixes the restored
> magnified mail window: actual multiline input, first Back and Close pass.
> Android202/preview210/shared1164/runtime288 and Java60 pass; macOS native-ui
> check and API31 packaging pass, not Windows tests. Source `8d50cb8a3` has34
> refreshed offline captures and both APK hashes. Phone-first HUD/touch, approved
> online login, public pack alignment and device gates remain open.
> [Current scope and evidence](generated/player-qa/native-android-shared-sync-20261001/README.md).

Status: shared player UI assembly and offline Android UI baseline, **not whole
Android UI acceptance or a completed online client**. Work is isolated on
`codex/android-shared-sync`; the original Android branch, checkout and Windows backend
are not edited.

## Current priority — playable flow first

User correction on 2026-09-09: finish the actual game flow before requesting
physical-device acceptance. Prioritize real login/roster/StartGame → shared
scene/bootstrap → authoritative movement and basic player actions. Whole phone
UI/input follows that playable flow; offline polish and memory diagnosis must
not displace it. Physical-device purchase/testing is deferred, not a blocker
to implementing the client. Approved live Gateway still required for live claims.

2026-09-10 keyed-map generation: recovered the exact handoff generator via the
GitHub connector (blob matches local tree), executed an unmodified target-only
mirror with full-pack network fallback disabled. Map0 parses as700x700; 4703
entries emitted and verified byte-identical to local PNGs. Whole-map references
7672 include2969 missing sources (existing generator budget, NOT completeness).
A conservative offline65x111 region around302,634 has565 references/6 missing.
No new runtime code/APK/render-ready. Evidence and exact six keys:
`generated/player-qa/native-android-keyed-map-20260910/README.md`.

Earlier 2026-09-10 local tile build: `build-local-map-atlas.mjs` reuses the existing Web
shelf packer and writes only a fresh external output root. Generated 49 pages
from 2111 exported raw-upload tile frames across 10 libraries (14785921 bytes).
All output PNGs decode; hash-name prefixes, dimensions and rect bounds pass.
Existing-output rejection passes; packer tests5 pass/1 absent-standard-manifest
skip. Outputs remain under Android target/local-world-20260910, outside Git.
Keyed objects, complete Bichon coverage, Android staging/rendering still open.
Evidence: `generated/player-qa/native-android-local-map-atlas-20260910/README.md`.

Earlier 2026-09-10 resource follow-up located existing starter entity atlas pages and
Bichon raw map in the original checkout (read-only). The new Android
`audit-world-assets.mjs` verifies 7 PNG page hashes/sizes/headers, 9650 rectangle
bounds and `0.map.gz` decompression. UI-only input fails as expected. This is
local integrity, not resource release provenance or Android render acceptance.
Derived map-atlas/keyed manifests remain absent. Current-branch Windows assets.rs
was not cached and its automatic promisor fetch timed out; the older original
checkout resolver was inspected only as a discovery hint, not substituted.
Evidence: `generated/player-qa/native-android-local-assets-20260910/README.md`.

Earlier 2026-09-10 local `ced102ba9` feeds shared MapModel/EntityModelSet ingress from
the same validated snapshot. Shared serde validates terrain/entities; map center
uses server sceneView or authoritative self position when the viewport is absent.
Android94/preview97/API31 target check pass. No placeholder render plugins are
enabled. Standard map-atlas/native-map-keyed manifests were absent in both checked
checkouts; Android staging contains only original-ui. Other authorized asset roots
still need checking. No new APK, scene render or live acceptance.
Evidence: `generated/player-qa/native-android-scene-models-20260910/README.md`.

Earlier 2026-09-10 local `0ee203649` adds exact-request shared runtime world-data
receipts. A real native-queue/Bevy-update regression proves that enqueue alone
has no receipt, coalesced latest data is actually applied, and invalid world
schema reports rejection without replacing valid state. Android ignores stale
request IDs and keeps StartingGame even after Applied; decode rejection resets
and disconnects. Runtime215 serial/Android93/preview96 and Android target check
pass. This closes world-data acknowledgement only, NOT assets/render readiness.
No APK or device run for this increment; the APK below remains source `af5acf6ad`.
Evidence: `generated/player-qa/native-android-world-receipt-20260910/README.md`.

Earlier 2026-09-10 local source `af5acf6ad` retains the complete immutable Gateway
world snapshot through Java/JNI and forwards a wire-shape projection to shared
runtime world/HUD ingress. StartGame/self identity gates remain; pending snapshots
are not exposed before acceptance or replayed by later position packets. Host
event sizes/queue memory are bounded, and session/map boundaries request shared
data/scene resets. Nullable wire scalars use shared HUD defaults. Java TLS8,
Android92 and preview95 pass. This is **ingress**, not verified runtime bootstrap:
map/entity asset producers, render-ready acknowledgement, incremental gameplay
packet routing and the playable loop remain open. No live Gateway or device
acceptance. Evidence: `generated/player-qa/native-android-snapshot-ingress-20260910/README.md`.

Earlier host-data bridge increment carries immutable server player/map/x/y in
GatewaySession.View and JNI JSON, validating into Rust HostState rather than
parsing a notice. Non-world phases clear it. Java TLS fixtures8/8, Android88/88,
preview91/91 pass. Initial direct Gradle invocation lacked ANDROID_HOME; rerun
with the existing SDK path passed. Logs `/tmp/android-world-bridge-{java,rust,preview}.log`.
This stores a partial server projection only: shared world/render bootstrap,
full packet/read-model/effect routing, real Gateway login and visible map are
NOT complete. No new APK/screenshot/live acceptance claimed in this increment.

## Implemented

- Local `bc392f889`: Android IME temporarily collapses mail attachment/gold
  presentation and moves the same shared Send/Cancel under the fields, with
  44 logical-pixel targets. IME dismissal restores details and geometry.
  Shared585/normal87/preview90, both APKs and API31 draft/attachment/Cancel
  short flow pass. Evidence:
  `generated/player-qa/native-android-mail-compact-20260909/README.md`.
  Longer run was killed by lowmemorykiller at ~1.2GB RSS; memory/stability
  and process-death recovery remain open, not hidden by the short-flow pass.
- Local `5b909ce45`: ChatSettings now switches Title/466→467 with the tab;
  FILTER/CHAT BOX image families match actual staged PNG labels and actions.
  Failure-first tests, shared584/normal86/preview89, both APKs and API31
  tab/transparent/Apply pass. Evidence:
  `generated/player-qa/native-android-chat-skin-20260909/README.md`.
  Phone target sizing and remaining settings presentation still open.
- `a6af7cb6d`: chat presses are consumed before a changed model rebuilds
  the tree. API31 tab/transparent/Apply now works; source transparent frames
  replace global alpha tint, and offline settings gets a proper shared draft.
  Shared583/normal86/preview89, both APKs pass. Redundant root Pass patches
  removed; no-op Android clamp guard retained. Evidence:
  `generated/player-qa/native-android-chat-input-20260909/README.md`.
  Settings skin parity and mobile whole-screen layout remain open.
- Local `f2246b476` separates the shared belt presentation group and anchors
  it bottom-left when the phone gutter fits, with source-layout fallback.
  Rotation/Close use shared controls; IME hides the layer and preserves its
  preference. Shared581/normal85/preview87/Java8 and both APKs pass; API31
  horizontal/vertical/IME/restore/Close inspected. Evidence:
  `generated/player-qa/native-android-belt-edge-20260909/README.md`.
  Empty-belt evidence only; phone-sized targets and whole gameplay masks open.
- Android enables `client-bevy/native-player-ui`: the exact shared shell,
  HUD/overlays, minimap, chat, notices and quest/NPC plugins used by Windows.
  `native-ui` remains the desktop superset with the audio playback backend.
  Typed UI sound intents are shared separately; this does not implement Android audio.
- Most shared layers retain the 1024×768 fit. `74883f8b3` independently
  anchors minimap image/frame/actions to the top-right safe edge; the panel
  launcher follows below it with 64×48 OS-logical targets. Bottom HUD/chat
  and dialogs are not yet phone-wide responsive layouts.
- One touch owner feeds existing Crystal pointer/drag handlers. Secondary
  fingers cannot inherit a released drag; focus loss releases the pointer.
  This is not a completed multi-touch movement/combat controller.
- Login, character name, change-password, chat/social drafts, mail text,
  map search, locked storage password and guild/trade amount fields connect
  to the OS IME. Draft edits are focus-guarded; shared validation/reducers
  are reused. Done sends one press/release pair through shared keyboard input.
- Back dismisses editing or local panels before the game menu. No local
  editing operation changes inventory, balances, authentication or world position.
- New-character preview now uses the source offset-aware 16-frame preview
  renderer. Crystal `NewCharacterDialog.cs:95-109` places the anchor at
  dialog +(120,250), with `UseOffSet=true`; the old stretched rectangle
  incorrectly covered the Create button.
- Gradle stages external PNGs from ChrSel, Prguse, Prguse2, Title, Items,
  Help, MMap and StateItem. APKs, generated resources and keys remain ignored.

## Offline verification surface

`MIR2_ANDROID_VARIANT=uiPreview` builds a separate
`com.mir2.web3.uipreview` package with compile-time `ui-preview`.
Normal debug/release variants cannot select fixtures using Activity extras.
The preview Java host refuses to connect, log in or StartGame. Every specimen
has an OFFLINE UI PREVIEW label; no fake Gateway login or bootstrap is emitted.

The 33 named specimens cover (including inventory amount and mail compose):

| Area | Specimens |
| --- | --- |
| Shell | login, empty roster, roster, create, password, SafeKey, delete confirmation, connecting, starting, disconnected |
| Player | HUD, inventory, character, skills, quests, options, platform settings, menu |
| Services/social | cash shop, NPC shop, mail, big map, storage, group, guild, accepted trade, chat settings, NPC |
| Other | death, focused chat/IME, help |

Inventory, storage, mail, skills, quests, NPC goods/dialog and trade include
explicitly offline specimens. An empty cash-shop/map view is **empty-state evidence**, not proof of populated
catalogues, pagination or server actions. Password/SafeKey capture restrictions
remain enabled; a protected screenshot is not visual acceptance.

Run `bash apps/game-client/platform-android/capture-ui-preview.sh OUTPUT_DIR`
after installing the preview APK. It waits for each specimen-ready marker,
captures a screenshot and process log, and fails on panic/FATAL/missing asset errors.

## Still open — do not mark the whole UI done

- `native-android-memory-baseline-20260909`: 33 cold-launch specimens pass;
  Help idle RSS ~303MiB, but repeated mail editing grows from 513→820MiB
  across 20 samples, then is killed again at 1171124KB RSS. Native heap
  growth is reproducible; exact allocation/lifetime cause is not diagnosed.
  Prior short-flow passes do not close stability. Instrument before fixing.
- `8423b220c` fixes ordinary/rich hint coordinate conversion under UiScale.
  The earlier "old stage clamp" diagnosis was imprecise: window coordinates
  were being scaled twice. Failure-first/system tests shared581,
  normal84/preview86, both APKs and API31 Mini Map/Mail hint screenshots pass.
  [Hint evidence](generated/player-qa/native-android-hint-scale-20260909/README.md).
  Cutout/IME-rich-hint and whole-screen interaction acceptance remain open.
- `74883f8b3`: first edge-layout slice, shared minimap group/image aligned
  at Android safe edge, shared Bevy button hits verified after moving.
  Expanded → collapse → Mail → compose/IME on API31 passes; shared579,
  normal84/preview86, Java8 and both APKs pass. Hover hints still use old
  coordinates; bottom HUD/chat/world and complete input masks remain open.
  [Edge evidence](generated/player-qa/native-android-minimap-edge-20260909/README.md).
- `ce08b88c9` wraps the inventory amount title into two lines, preserving
  the specimen item name above icon/input. Failure-first layout test,
  shared578/normal83/preview85, both APK builds and API31 IME/Cancel pass.
  [Amount title evidence](generated/player-qa/native-android-amount-title-20260909/README.md).
  Longer-than-two-line names, localization and phone-wide layout remain open.
- `08f658a53` fixes shared Help text-row/footer overlap without deleting
  content or changing pages. Failure-first geometry test, shared577,
  normal83/preview85 and API 31 page1 → page2 touch/screenshots pass.
  [Help evidence](generated/player-qa/native-android-help-rows-20260909/README.md).
  Phone-specific guidance, target sizes and full-screen layout remain open.
- Offline refresh at `a8bb81954`: 33/33 specimen captures/log checks,
  shared576/normal83/preview85 and preview APK build pass. This is smoke,
  not all-interaction acceptance. Visual sampling retains centered 4:3 HUD,
  Help last-row/footer overlap, clipped amount title and mail action area
  obscured by IME. Full-screen layout and these usability gaps stay open.
  [Refresh evidence](generated/player-qa/native-android-33-refresh-20260909/README.md).
- Test-only `7770110db` verifies existing shared session reset from Android:
  Login/ConnectionLost clears ordinary player intents/pending/drafts on the
  next update, without reusing storage request IDs. normal83/preview85,
  shared pending31 and normal APK pass. No duplicate reset implementation.
  [Reset audit](generated/player-qa/native-android-reset-integration-20260909/README.md).
  Live generations, unknown outcomes and real account transitions remain open.
- Local source `13f001d0b` discards unsent shared Gateway effects at disconnect,
  reconnect start, logout and inactive/unfocused boundaries, preserving local effects.
  API 31 queued revive → Home → resume logs one discarded command; normal82/
  preview84 and both APK gates pass. Other queues/in-flight/generation wiring remain.
  [Session evidence](generated/player-qa/native-android-ui-session-20260909/README.md).
- Local source `06a5dea1a` adds death-only Android Revive touch action.
  API 31 touch emits a shared TownRevive intent, without restoring HP;
  alive state hides the target. normal80/preview82 and both APK gates pass.
  [Revive evidence](generated/player-qa/native-android-revive-touch-20260909/README.md).
  This is UI-to-queue only: real socket/receipt and lifecycle wiring remain open.
- Local source `bed749479` adds a guarded notice-body IME touch target.
  API 31 repeated Back/retap retains the draft; Cancel restores the notice.
  Shared576, normal78/preview80 and both APK gates pass. Live publication
  and physical-device acceptance remain open; source is local only.
  [Retap evidence](generated/player-qa/native-android-guild-retap-20260909/README.md).
- Local source `77edc7afb` fixes guild notice IME panning text offscreen.
  API 31 eight-line input and Back/Cancel are verified, normal78/preview80
  and both APK gates pass. Retap is addressed above; live publication remains open.
  [Guild evidence](generated/player-qa/native-android-guild-ime-20260909/README.md).
  Remote remains Draft `99a1cd121`; this source is local only.
- Local source `ac1a37d55` fixes touch rail Close leaving Help open, using the
  shared complete-close method. API 31 before/after replay and host tests
  76/76 normal, 78/78 preview pass; modal/inactive guards remain intact.
  [Close evidence](generated/player-qa/native-android-rail-close-20260908/README.md).
  Read-only remote check remains `99a1cd121`, Draft; no push retry this round.
- Latest local editor source `fafbc716f` adds trade/guild amount IME geometry
  and multiline mail/notice input. Trade amount reopening/cancel and two-line
  mail text have API 31 evidence; guild-specific populated cases remain open.
  Android tests 74/74 normal, 76/76 preview; shared UI 575/575. These later
  commits/evidence are not yet confirmed remote: PR head last verified `99a1cd121`
  after HTTP 408/SSH push failures. [Latest editor evidence](generated/player-qa/native-android-editors-20260908/README.md).
- Source `79d6d7ce2` fixes GameActivity Back bypass, shared modal cancellation,
  same-field IME reopening and inventory amount bounds. Source `99a1cd121`
  bounds mail compose with six-row attachment paging. API 31 amount/recipient
  input, Back priority, Help close and page-two attachment selection have
  targeted evidence. Other nested fields and multiline IME are still open.
  [IME and mail evidence](generated/player-qa/native-android-ime-20260908/README.md).
- The two initial touch gaps have targeted API 31 regression evidence at
  source `62344b0e5`: same-frame tap edges and hit-test ordering are fixed;
  bag/help keep a 24 OS-logical-pixel top gutter. Bag dragging and short-tap
  inspect → Panels → Close now pass. See
  [touch regression evidence](generated/player-qa/native-android-touch-20260908/README.md).
  This does not establish every window's touch behavior or physical-device acceptance.
- The real login socket still lacks the shared gameplay bootstrap/read-model
  projection and complete gameplay/transaction intent dispatch. Real StartGame
  remains on the transition screen, rather than fabricating a playable map.
- Character creation/deletion/password changes and gameplay operations need
  actual request/result wiring and an approved Gateway test environment.
- Populated quests/skills/shops/maps, all nested dialogs and item operations
  need scenario-level interaction evidence, not merely a screenshot of a root panel.
- Dedicated virtual joystick, multi-touch combat/skill controls and the actual
  revival round trip require further Android work; desktop keyboard affordances
  in the shared UI do not prove phone usability.
- Android audio, exact Crystal visual/feel parity, physical-device touch/IME,
  lifecycle/reconnect, low-end performance, signing and human acceptance remain open.
- The original chat frame PNG is white in this source pack. This work does
  not silently recolour source assets or claim original-client A/B acceptance.
- Full Windows audio-enabled regression is not established here: this local
  crates.io cache lacks Bevy audio 0.19. Shared visual-feature tests are a
  separate denominator.

Evidence: [Android player-UI baseline](generated/player-qa/native-android-player-ui-20260908/README.md).
No production deployment, real account login or save mutation was performed.
