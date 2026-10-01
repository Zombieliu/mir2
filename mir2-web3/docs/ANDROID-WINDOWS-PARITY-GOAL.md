# Android 对齐当前 Windows 完整度 goal

建立：2026-10-01。状态：**Active，尚未完成**。

本轮NI-09源码：提取冻结Windows相同Chat/ObjectChat转换，Java认证owner快照
之后才转发公共聊天；Android有界FIFO32/128KiB、顺序/失败后缀重试/切图保留/
终止清理通过。Android267/preview281/shared1205+8既有ignore/Java37+37/API31两
变体/Mac聊天2及函数等价核验通过，独立只读复审无阻断。v14尚未构建、安装或
画面验收；现有共享runtime在critical/ACK压力下仍可驱逐聊天，不宣称端到端无丢失。
下一步干净提交双包与实际接收聊天画面，然后手机HUD负重、NPC/服务及剩余完整
Windows分母；在线与真机仍OPEN，goal不结束。
[本轮源码及失败先行](generated/player-qa/native-android-received-chat-20261001/README.md)。

最新实际v13：干净cfac6ecd4双包/hash/安装、每包6647原图/三份metadata及
四张冻结Windows原图字节核验通过；模拟器BAG原471重量条恢复，余重34/
金币12352可见，三组采集进程日志无缺图或错误。手机HUD负重显示仍未验收，
保留首次启动超时、模拟器宿主崩溃及拒绝上传后的SDK临时dump移除记录。
最新推送仍失败，远端核验只到f04a74399；下一叶子NI-09聊天/HUD负重，再
继续NPC/服务和完整动作分母。完整UI/资源、在线及真机仍OPEN，goal继续Active。
[精确v13与失败边界](generated/player-qa/native-android-weight-bars-20261001/README.md)。

此前重量条源码检查点（在v13实包之前）：补入冻结Windows470..473原图，旧源包13240文件未变；
稀疏资源门20/20、物品13/13、技能图标7/7、Java36+36及暂存6647图/三份metadata
字节核验通过，独立只读复审无阻断。v13尚未构建或显示验收；下一步精确双包与
模拟器原重量条，再继续完整聊天/NPC/服务/动作分母。v12证据9e29d7b67仍本地，
本轮推送传输失败，实际PR/远端仍f04a74399。在线、真机和完整goal未完成。
[源码门及失败](generated/player-qa/native-android-weight-bars-20261001/README.md)。

最新实际v12：干净f04a74399双包/hash/版本/安装及每包6643原图/两份metadata
字节核验通过。模拟器StatsI/II显示HP/MP/属性/经验/三项重量，BAG与HUD金币
同为12352、剩余重量34、两窗关闭有响应。实际暴露UI_32bit/471/473重量条
缺图，正常/人物启动仍超时且保留Launcher ANR；不能算完整UI或在线。下一叶子
先补冻结Windows的470..473原图打包门，再继续聊天/NPC等完整分母。真机仍缺。
[精确包、实际数值与失败](generated/player-qa/native-android-player-ingress-20261001/README.md)。

此前 NI-02/08 源码检查点（在v12实包之前）：复用冻结Windows的同一人物属性/钱包游标，接通
已知权重/完整u32显示、部分快照保留及四类公开余额增量；Java前置owner/Hero/
精确整数门与同批拒绝后的宿主终止门复现并修复。Android257/preview270/
shared1203+8ignored/Java36+36/相关Mac源码6项/API31双feature通过，最终有界
源码复核无P0/P1/P2阻断。v12仅设置、尚未构建或显示验收；下一步精确包/模拟器
属性数值，再继续聊天/服务/整套动作。完整分母、在线与真机仍OPEN。
[源码及失败保留](generated/player-qa/native-android-player-ingress-20261001/README.md)。

当前 G4 有界实测：干净6fe6a17ac双包v11/hash/安装与6643 PNG、两份metadata逐字节
核验；模拟器背包／人物窗不再遮 HUD/摇杆，四个人物页和关闭实际点击有响应，
保留六个快捷格。Android249/preview260/shared1203+8ignored/API31两种源码门通过。
启动仍有15844/13174ms超时；属性预览缺crystal_stats、所有48dp/拖拽/短屏/登录IME/
服务/在线/真机仍OPEN。下一叶子接真实属性/余额到共享UI，完整Windows分母不变。
[精确v11、原始截图与失败保留](generated/player-qa/native-android-phone-panels-20261001/README.md)。

发布检查点：最新NI-02/08源码已正常推送至 Android 分支 f04a74399，
远端引用和 PR253 同 head 已核验；PR仍Open/Draft、base不变。不回推Windows，
不上传APK/原素材/密钥，不合并或部署；APK源码仍是各自记录的精确提交。

最新实际 v10 检查点：干净源码452398d4c双包/版本/hash/安装核验，每包6195物品
PNG与两份metadata逐字节一致；模拟器实际摘要吻合，离线背包原图/数量与人物
原剑帧可见。普通启动仍timeout12721ms；背包遮状态/聊天/摇杆，人物/技能/登录
窗仍小，手机布局不通过。源资源叶子不等于完整UI或在线完成；下一轮修手机窗口
布局/触控，保留服务/完整分母/资源/性能/联网/真机缺项。
[精确v10及实际布局失败](generated/player-qa/native-android-item-geometry-20261001/README.md)。

最新 NI-05 原图 geometry 源码：Android 从 APK 读取两份有界物品元数据，
保留 Items 原尺寸与 StateItem 原偏移，失败不补假值；预览样本也通过同一背包
适配器。Android244/preview255/Java33+33/普通与实际预览API31通过，13个真实
打包正反例与7个技能图标旧门回归通过，源文件不变。v10版本已设置但尚未构建，
实际背包/装备画面、手机操作和在线都未验收；此前源码与失败记录保持独立。
[原图源码/暂存证据](generated/player-qa/native-android-item-geometry-20261001/README.md)。

最新有界源码进展：NI-07 复用 Windows 的同一技能游标/投影，接通 typed
learned skills、精确毫秒、owner 成功回执和 exact-request 按键结果；同角色
切图保留个人状态，不恢复旧地图。Android227/preview236/shared1196+8ignored/
Java32+32/API31 通过。精确源码8eb1a7d34的v8包已构建/安装，离线SPELLS显示
FireBall/F1，但技能图标缺失并重复加载，图像验收FAIL；v7不包含这次源码。
真实在线、完整UI/资源和真机都未通过。[证据](generated/player-qa/native-android-skill-ingress-20261001/README.md)。

随后补齐技能图标打包门：新本地包的448帧像素校验通过，旧12,789文件不变；
不完整v8输入会被打包检查拒绝；复现并修复重复帧可过门的P2，唯一编号/完整
覆盖/路径校验的真实Gradle正反例7/7通过，源资源不变。preview237/Java32+32通过。
干净源码394307db8的v9双包/版本/hash/安装已核验，各448帧实际APK字节一致；
模拟器直接SPELLS显示原图标/Lv1/F1，捕获日志无图标错误。两次启动等待超时
保留，完整手机窗/性能/联网/真机仍未通过。
[资源与精确包](generated/player-qa/native-android-skill-icons-20261001/README.md)。

NI-05/06 源码接通：提取复用 Windows 的 typed 背包/装备/快捷栏/提示投影和
冻结七类回执，保留同角色切图、终止清理与16条FIFO背压；12函数等价核对通过。
Android236/preview246/shared1203+8既有ignore/Java33+33/API31通过；Mac桌面
inventory筛选6通过/2资源依赖失败，未宣称全绿。Android两类物品geometry仍None，
实际JNI/新APK/在线/触屏均未验收；v9不包含本轮背包源码。下一步补原图尺寸/偏移。
[本轮源码与失败证据](generated/player-qa/native-android-inventory-ingress-20261001/README.md)。

本轮用户要求把 Android 完整度对齐当前 Windows，而不再把登录小切片或
离线 UI 展示当作终点。目标是原生 Bevy/GameActivity Android；Capacitor
或远程旧 Web 页面不替代本 goal 的交付。

## 1. 冻结基线与隔离

| 项目 | 本轮核验结果 |
| --- | --- |
| Windows 当前源码基线 | `codex/playtest-registration`，`3f5e61533235921369bc13a7760b4a56b0e467e5` |
| Windows 基线提交 | 2026-10-01 09:54:27 +08:00，技能节奏和非模态窗口输入修复 |
| 旧 Windows 来源 | `codex/windows-player-journey`，`6ae080711fc7b3aaf06dd9c6bcf63f121682dbe4`，PR #250 仍 Draft；它是上述基线的祖先，不是当前终点 |
| Android 执行分支 | 独立 `codex/android-shared-sync`，PR #253 仍 Draft，base 不变 |
| Android 建立 goal 时 HEAD | `b8398db406b4d96d7d4f8381373e404fc0da9553`，当时工作区干净 |
| 两端已共有的祖先 | `9476f3845e89b9ede164fe18e9c353c1e44f7626` |
| 尚未进入 Android 的 Windows 提交 | `fefd18370`、`6226bccce`、`3f5e61533`；Git 两侧独有提交数 165 / 3，不是功能完成度 |
| 最新已交付 Android APK 源码 | `97411004114903a21963e82e2790bdafd4578962`；v5，Debug 与离线 uiPreview 分开 |
| 原工作区 | `codex/steam-main` / `31b2b396057d82aa80567a33a668907d5d41a432`，保留其已有修改 |
| 原 Android 分支 | `codex/android-player-journey` / `5d417ca75a5fe845a7d6f6d9e16c720d55e19b36`，不改写 |
| 当前连接设备 | 仅 `emulator-5554`，`sdk_gphone64_arm64`；没有已连接真机 |

GitHub PR 与远端引用已只读核验，并 fetch 两个 Windows 来源引用。没有合并、
切换原工作区、stash、reset、clean、force push、合并 PR 或部署。
本文件路径相对仓库根；任何本机资源路径都不得照抄 Windows 盘符。

Windows 的三个层级不能混用：当前源码是 `3f5e61533`；Windows 文档记录的
r7 Candidate/安装包源码是 `fefd18370`；技能修复报告当时用户实际 F 盘安装
仍为 r5 `c7eacee12`。这里选择最新源码能力作为 Android 对齐基线；这些
Windows 安装状态是对应报告的观察，不是本轮在 Windows 机器上重新验收。

若上游继续变化，先完成这个冻结快照；新增差异单独记录，不能悄悄变更分母。
不得把 Windows 尚未验收的源码能力标成 Android 已验收，也不要求本 goal
顺带修完 Windows 自己的全部遗留问题。

## 2. 完成标准

Android 必须具有 Windows 基线已有的玩家功能、共享状态和协议能力，且提供
可在手机上操作的等价入口。共享 UI 指共享业务语义、状态、动作和素材来源，
不是把桌面 1024×768 画布整体缩小到手机。

- 完整共享 UI / 窗口能力、九语言呈现、手机布局与输入；不另写一套游戏规则。
- 同版本资源覆盖、完整地图/对象层/人物装备动画/特效/音频能力；缺帧不补假图。
- 真实认证、角色与地图流程，真实服务端数据接到每个已支持共享 read model。
- 移动、战斗、拾取、背包、NPC、任务、保存与恢复达到 Windows 已有能力。
- 当前 Windows 有的社交、邮件、仓库、交易等功能逐项审计，不仅打开空窗口。
- Android 安装、版本/资源完整性、更新路径具备平台等价交付；正式签名/发布
  在获得相应授权和材料后单独验收。
- 构建、模拟器、真实在线流程、真机和人工接受分列，缺项不能计为通过。

Win32 分辨率、Inno 安装器、EXE 更新器不原样搬到 Android：对应的是手机
全屏/安全区、APK 安装、可信版本更新与资源缓存。需要本机或平台专用实现的
部分必须保留同等能力，不借“平台差异”删减普通玩家功能。

目标完成需要所有适用验收叶子有证据；仅有外部 blocker 时仍为未完成。
真机或人工接受缺失不自动降级成完整交付。不能因为大部分测试通过就宣布
100% Accepted，也不能把 Git 提交距离、截图数量或单测数量当成功能百分比。

## 3. 初始能力差距表

以下是本轮执行目录，不是已冻结的百分比分母。下一轮须逐项展开 Windows
实际入口、Android 发送/接收路径、验收脚本和证据，再建立完整叶子清单。
“已有共享/局部证据”均不表示 Android 真实联网通过。

| ID | 对齐能力 | Android 当前边界与待办 |
| --- | --- | --- |
| AP-01 | 最新共享源码 / 依赖 / 协议 | Android 保有此前共享集成；上述三个 Windows 后续提交未集成。先正常 merge 并保留 Android 专属适配，不复制服务端规则 |
| AP-02 | 登录、注册、改密、安全提示 | 共享表单已挂载；Java 有正常凭据 WSS 登录宿主。注册与其他账户动作逐入口审计，真实账号验收未做 |
| AP-03 | 角色列表、创建/删除/选择、StartGame | 已有状态机与共享 UI；真实账号、角色和服务端初始位置尚未验收，不用离线角色冒充 |
| AP-04 | 地图资源 / 加载 / render-ready 切场 | 仅有比奇局部 849-draw 证据；现有导出仍缺 2969 / 7672 源帧引用。审计源端 no-draw 与实际缺失，不能视为全图齐备 |
| AP-05 | 角色/怪物/NPC/掉落完整对象层 | 已有 bounded packet/render 路径和本地 Archer/mount proof pack；全职业/性别/装备/坐骑/地图资源与正式 pack 版本仍需对齐 |
| AP-06 | 世界/HUD/个人状态增量与回执 | 现有世界 snapshot、对象流和部分交易回执已接；其余普通玩家 packet 到共享 reducer/read model 逐项补齐，不能依靠周期快照掩盖遗漏 |
| AP-07 | 普通移动、转向、走跑、门/地图转移 | 共享意图和对象位置已有；需真实 Zone 权威移动、碰撞/修正、保存后重进位置证据；不发送 MoveTo/debug teleport |
| AP-08 | 三职业既有战斗/技能/冷却/死亡复活 | 已有部分表现与动作入口；补最新 owner ACK/单调时钟冷却与输入规则，验证真回执、目标选择和三职业入口，不算三职业完整成长验收 |
| AP-09 | 掉落拾取 / 背包 / 穿脱 / 快捷栏 | 共享动作及 DNItems 来源已有；每种已支持移动/拆分/合并/丢弃/使用/穿脱操作需真状态与触屏证据 |
| AP-10 | NPC 对话、商店、修理、仓库服务 | 有共享界面；服务上下文、任务 NPC 入口、购买/卖出/修理与货币/物品回执逐项核对，不能只证明空窗或本地按钮点击 |
| AP-11 | 任务/日记/详情/寻路/既有新人流程 | 有共享 UI；绑定真实任务状态，保留非模态移动/战斗；以正常账号证明基线已有路线，保留 Windows 未接受的 caster 门槛 |
| AP-12 | 仓库 / 锁定 / 密码 / 扩展 / 存取 | 有共享窗与部分 exact-request 收据；完整数据、IME、槽位触控和失败/未知结果行为需在线验收 |
| AP-13 | 聊天 / 设置 / 邮件 / 组队 / 行会 / 交易等 | HUD/chat/设置/本地邮件草稿有实测；真实收发、目录/通知/金额/物品及其余 Windows 已支持动作仍需逐项核对 |
| AP-14 | 手机整套窗口、最小地图/大地图/选项/帮助 | 当前 v5 仅完成 bounded HUD/chat/belt；其他窗仍有桌面 magnification。逐窗进行短屏/安全区/滚动/触控适配 |
| AP-15 | 非模态窗口与输入捕获 | Windows 最新修复普通窗口不冻结行动；Android 还需审计面板、拖拽、IME、商务/NPC 模态与多指所有权，保留正确隔离 |
| AP-16 | 九语言 / 字体 / 文本与输入 | 共享语言数据已有；Android 九语言真实渲染、RTL/复杂字形、长文本、软键盘及本地偏好尚未整端验收 |
| AP-17 | 动画 / 特效 / 声音 / 光照 | 有 bounded atlas/action/effect 表现；完整源帧、专用 spell 分支、Android 音频与焦点需核对。native-player-ui 不等于 Windows 音频 backend 已接入 |
| AP-18 | 断线/恢复/退出/保存/会话隔离 | 当前背景会关 socket 并要求重新登录；旧命令不得重放。须核对实际 nativeResumeV1 协商/宿主，不凭 Rust helper capability JSON 宣称已完成恢复 |
| AP-19 | 资源按需缓存 / 内存 / GPU / 稳定性 | 局部页加载和预算已有；完整场景、反复编辑/切图、低内存/进程回收与持续运行证据尚缺；历史内存失败不能抹掉 |
| AP-20 | 安装 / 升级 / 版本与资源完整性 | v5 是诊断 Debug/uiPreview APK，不是 store release；需可重复安装包、源码/Gateway/资源版本、SHA-256 与可信更新方案及验证 |
| AP-21 | 在线跨端一致性与真机验收 | 当前仅模拟器连接；真实 Android 流程、Windows/Android 同服可见交互及物理设备触控/性能未做。不得把 Mac 或 Windows 测试当 Android 证据 |

凡 AP-13 中 Windows 已支持但表中未单列的模块，都要在叶子审计中展开；不能
因表格省略了名字就默认为不在范围。Windows 自己未支持的玩法不要求 Android
独立实现。将读到的历史 README 说法与当前代码分开，发现过时条目后再修文档。

## 4. 执行队列

- [x] G0 — 正式 goal 建立；只读核验真实仓库、远端、隔离状态、当前 Windows
  来源 SHA 与设备列表；形成此初始能力目录。不是完整代码/功能审计通过。
- [ ] G1 — 将三个已冻结 Windows 更新集成进独立 Android 分支；逐文件处理
  shared chat / metrics / overlays / quest / skill 冲突，保留 Android HUD/IME
  与 GLES 适配；建立双方能力叶子清单并跑受影响共享、Android、Java 与构建门禁。
  - [x] G1-source — 正常合并冻结来源，保留 Android native-player-ui/HUD/IME/GLES，
    接入非模态手机操作保护并完成源码回归；详见下方本轮记录。
  - [x] G1-ingress — 已建立20项宿主接收子目录与6项输入集成检查；它不是完整
    玩家功能百分比分母，仍须逐项展开每个服务的动作/回执/窗口与失败验收。
  - [x] G1-package — 精确源码10bf437f2的v7诊断APK/版本/hash与模拟器离线
    菜单/隐藏摇杆/共享背包重新验证；在线、完整UI和稳定性不在此勾选中。
- [ ] G2 — 真实登录 → 角色列表 → 创建/选择 → StartGame → 服务端地图/位置。
  审计 transport、认证和 render-ready，不发裸 account_id 冒充身份。
- [ ] G3 — 完整资源和对象/地图生命周期、真实权威移动、战斗、拾取、背包、
  NPC/任务/仓库/社交等已支持 read model 与命令/回执闭环；按高依赖顺序小步完成。
- [ ] G4 — 手机全套窗口/九语言/输入、普通面板非模态、IME、双指走跑战斗、
  音频/缓存/性能；可在 G2 外部条件等待时先做不依赖在线账号的有界叶子。
- [ ] G5 — 同一精确 APK 上完成 API31 真实在线回归、跨端交互、真实设备
  触控/后台/断网/内存/渲染测量与可安装交付。人工接受和正式分发各有明确门槛。
- [ ] G6 — 回查全部适用叶子；交付 APK+hash、版本、设备、证据和遗留限制。
  未达到目标不得把 goal 标 Complete。

每个叶子依次记录：源码接线、focused/shared regression、Android target/build、
模拟器离线/在线、真机、人工接受。采用 `PASS / FAIL / OPEN / EXTERNAL`，
其中 EXTERNAL 仍是未完成，不用虚假的完成百分比。测试重叠/ignore 分列。
后续补出完整分母才可以报告“已通过叶子数 / 适用叶子数”，不得先拍百分比。

## 5. 写集、权限与并发边界

优先 `apps/game-client/platform-android/**` 与 Android 专用 QA/goal 文档。
G1 是明确的跨分支集成，不向 Windows 分支回推；保留已有共享源实现，只处理
Android 所需接线和兼容性。后续任何 shared client/runtime 改动前声明具体文件；
`runtime.rs` 等高冲突文件每轮只允许一位 writer。

建立 goal 本轮只写本文和 `docs/AGENT-TASK-QUEUE.md` 的 goal 指针，不修改
游戏代码、Windows/Gateway/Simulation、现有运行服务、账号或资源库。
PR #253 保持独立 Draft；不自动改 base、不合并 PR。分阶段验证后再提交推送；
源码、证据与 APK 各自绑定完整 SHA。APK/缓存/密码/令牌/签名密钥不入 Git。

共享 Zone 和个人 Session 保持职责分离；客户端只发普通玩家意图。不得复制
手机端权威战斗/掉落/交易/存档规则，不暴露 QA/admin/debug 指令，不绕过认证，
不启用 demo 回退，不重放未知结果的购买/交易等非幂等命令。

## 6. 外部验收材料（不阻止其余安全代码工作）

- Android 真实联网需获准的测试 Gateway（WSS/相关 HTTPS）、服务器版本和
  普通测试账号。v5 Debug build 当前明确为空地址，uiPreview 禁止联网。
  Windows 报告存在 `/playtest/ws` 不自动构成本轮访问/数据写入许可。
- 凭据由用户在本地输入；不得从其他工作区、日志或旧文件搜密码/令牌。
- 完整、获准的资源来源/版本仍需核对；现有 proof pack 不算正式完整共享包。
  不未经许可下载/重发整套素材，不改线上 immutable release。
- 当前无已连接真机；可继续模拟器/构建/代码，不声明物理设备通过。
- 正式 APK 签名、可信更新发布和商店/公网分发需要额外授权与材料；不为了
  此 goal 部署生产、修改真实存档、改服务器 auth/rate limits 或搬用 Windows 密钥。

当一个叶子需要这些材料时列出最小缺项；其他有界离线叶子继续推进。
本轮没有进入真实账号、买卖、战斗、保存、资源发布或物理设备流程。

## 7. 已核验证据与恢复入口

- [Android v5 exact-source/APK/UI evidence](generated/player-qa/native-android-phone-ui-20261001/README.md)
- [Earlier Android/shared integration](generated/player-qa/native-android-shared-sync-20261001/README.md)
- [Android UI coverage and historical failures](ANDROID-UI-COVERAGE.md)
- Windows 冻结源 `3f5e61533` 下：
  `docs/generated/player-qa/skill-input-shop-20261001/README.md`（shared1186 /
  Windows801、Zone178、Gateway focused8+13；不冒充最终全后端 green）及
  `docs/generated/player-qa/native-updater-20261001/README.md`、
  `docs/generated/player-qa/native-display-20261001/README.md`。
  这些上游文件在 G1 前以冻结 Git source 阅读，不假定尚未 merge 的本地副本是最新。
- `git merge-base`、两侧 log/diff、GitHub PR state/head/base、`git ls-remote` 和
  `adb devices -l` 支持本轮基线记录；本轮尚未重新运行游戏测试/构建。

恢复时先读本文和 `AGENT-TASK-QUEUE.md` 顶部，再确认 goal 状态、工作区状态
和实际待办。保留旧失败和独立 Windows/CI/三职业/容量门槛；容量 goal 不恢复。

## 8. G1 源码集成记录（APK/在线门槛尚未通过）

普通 merge 导入冻结 Windows 来源的三个提交，不改写 Android 或 Windows 历史。
13处文本冲突已逐项合并；保留手机 HUD/chat/IME、native-player-ui 和 GLES 清理。
共享显示纯类型允许手机 UI 编译，但 Android 不安装桌面分辨率设置资源或插件，
不打开桌面音频特性。两个桌面 match 明确处理共享 Hero 类型，无新增攻击/NPC规则。

Android 普通面板不再统一隐藏 HUD/摇杆；持续和新摇杆操作均有回归。真正的
NPC服务、notice、任务确认、技能分配、聊天编辑和死亡仍保护动作；切换窗口
释放旧 UI pointer，避免第二根手指误点新窗。死亡继续保留 Revive 入口。
宿主真正未接通的数据列在 [ingress差距](ANDROID-NATIVE-HOST-GAPS.md)，其中
技能/owner cast/精确冷却仍 OPEN，不能因 merge 共享技能 UI 就宣称已完成。

本轮离线源码门禁：Android217/217、uiPreview225/225、shared native-player-ui
1195通过/8既有ignore、runtime290通过/1既有ignore、Java两个variant各30/30、
API31 arm64 target check、Zone178/178、Gateway技能focused8/8；计数相互重叠。
Mac桌面宿主相关focused为非模态8/8、Hero cursor1/1、hover保护1/1。
Mac完整桌面源码测试为622通过/180失败/5ignore，含完整Candidate素材、Windows
字体与Mac临时路径祖先等环境失败。保留完整日志，不把focused替换成全量green，
不宣称本轮Windows实机通过。服务端也只有上述focused/Zone范围，不称全后端green。

失败证据保留在 Android `target/windows-goal-g1-20261001`：非模态先复现0/2，
中间Android215/216；新增共享显示测试的桌面音频依赖错误；稀疏checkout缺少
已有SQL/配置/编译fixture；Mac桌面全量结果。已有SQL只补出为编译输入，没有
执行迁移或连接数据库。继承Windows的原始日志字节未修剪；它们的尾部空行
使未过滤cached diff check非零，源文件/文档的scoped diff check和format check通过。

v6版本号已区分旧包，但APK打包/安装/截图仍待下一步精确绑定本次源提交。
没有配置真实Gateway、输入账号、写存档、部署服务或获得物理设备证据。
Goal保持Active；G1整体和G2–G6仍未完成。

## 9. v6 实测发现与 v7 手机菜单修复

v6源码`b7caac732ac31a31353b69e8b0bb956ada3f583a`为正常merge，父提交
`1e98c89b6`与冻结Windows`3f5e61533`。两个原生API31 APK构建/保数据安装成功，
Gateway显式为空；uiPreview禁止联网。实际世界帧为849 draws/8 entities/
13 entity layers/2 effects，仍只是此前局部proof pack，不是完整比奇或在线验收。

首次截图被System UI无响应框遮挡；原图保留，`dumpsys activity lastanr`为
`<no ANR has occurred since boot>`，不能据此宣称原因已确定。点击Wait后焦点和
原生世界画面恢复。此后只串行运行一个测试variant，不清数据/重置AVD。
菜单与背包已实际点开，背包打开后摇杆产生离线意图；另发现展开手机菜单时，
视觉已隐藏的摇杆仍响应原区域滑动。v6因此只留作中间包/失败证据，不作为最终
本阶段通过包。整个模拟器/性能稳定性并未因Wait恢复就获得接受。

v7只修改Android手机控制与版本标识：展开rail时取消/拒绝隐藏摇杆，rail状态
加入TouchContext以防旧手指误点新的菜单项。新失败回归先复现0/1；修复后
普通Android219/219、uiPreview227/227通过，另有旧pointer释放与抬指恢复测试。
共享/Zone/Gateway源码不变，前一阶段证据仍按其原范围保留。v7精确源码APK与
实际重新验证仍待下一步，不把v6画面挪作v7证据。

## 10. v7 精确包与有界模拟器门禁

源码`10bf437f2d99b09a850fc3359df5172658215681`干净提交后构建两个原生APK，
versionCode7/versionName0.1.4-windows-g1。Debug385029150bytes/hash1cb092026a
与uiPreview388672390bytes/hashec909e12cb的完整hash、资源/设备/日志详见
[G1证据](generated/player-qa/native-android-windows-g1-20261001/README.md)。
实际安装显示正确版本；Debug共享登录明确未配置测试服务，uiPreview世界帧
重新采集。菜单展开时隐藏摇杆滑动0新增意图；打开共享背包后可发送两个离线
移动意图，不冒充权威移动。两秒回前台黑帧保留，之后同PID世界恢复；模拟器
帧间隔偏高，性能/稳定性/断线恢复并未通过。v6的SystemUI失败仍在证据中。

G1-package仅按上述有界范围勾选；G1整体仍OPEN，因为完整功能/动作/回执
验收叶子需要继续展开。技能、背包、NPC/任务/邮件/社交等typed ingress未接通
的条目不因窗口共享而变成完成。下一步NI-07技能/owner ACK/精确冷却与隔离
回归；获准线上地址/普通账号未提供时继续其他安全代码工作。Goal仍Active。
