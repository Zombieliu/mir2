# WN-UI-FUNC-01 — Windows Native 全页面与按钮功能验收

> 状态：`[ ] OPEN — non-visual implementation gates green; human/real-window acceptance pending`
> 目标：Windows 原生客户端中每个可见页面、按钮、输入框、列表项和快捷键都与 Crystal 原版具有一致的可见结果与服务端结果。
> 原则：页面能显示不等于功能完成；按钮有贴图不等于按钮可用；自动登录、环境变量和直接发包不能替代真实鼠标/键盘验收。

> 2026-08-21 R1 集成：账号改密、删除确认、Safe Key、本地共享
> `ui-core`、Mail/Shop/Storage 严格数据入口及 Android 主机骨架已落地，
> 自动化门禁通过。Options 的运行时 effect consumer、Big Map、Chat
> Settings、Android 真机和真人鼠标证据仍未完成，因此本清单保持 OPEN。
> 详见 `docs/generated/player-qa/native-ui-controls/R1-INTEGRATION-REPORT.md`。

> 2026-08-21 R2 非视觉集成：Options 已接真实窗口模式与配置持久化；
> Chat Settings 已改为共享状态唯一真源并接本地持久化；Big Map 服务端
> `WorldMapSetup/NewMapInfo/SearchMapResult` 已完成。当前 WorldMap 权威配置
> 为禁用且没有可传送 NPC，故传送保持无状态变化的安全 no-op。原生 Big
> Map 页面、光照、截图和真人鼠标验收按用户要求暂停，本清单仍保持 OPEN。
> 详见 `docs/generated/player-qa/native-ui-controls/R2-INTEGRATION-REPORT.md`。

> 2026-08-22 R4 非视觉收口：本轮已接入 BigMap native adapter、七项 Options
> runtime hooks、authoritative Observe、Change Password 服务端 ack、native
> lighting，以及 skill binding 原子持久化。当前自动化门禁为：ui-core 36/36、
> client default 101/101、client native 318/318、Windows 264/264、runtime
> 179/179、Android 44/44、Web typecheck 与 component controls 2/2。当前
> registry 为 141，placeholder 为 0；3 条 no-op 仅对应 Credits、Light frame
> visual，以及 9 个 Crystal source-disabled menu family，不能按缺失功能计数。
> 证据仍只证明代码/数据流门禁，不证明真实窗口视觉或真人接受。

> 2026-08-21 R3 非视觉收口：Windows Options 已接真实 Bevy/WAV 音频后端，
> 音乐、音效开关和 0..100 音量独立生效；发布包显式校验并携带合法的
> `Login2.wav`/`Select2.wav` 回退素材。World Map 改为运行时读取权威
> `WorldMap.ini`，共享 Zone `TeleportToNpc` 已实现费用、NPC、碰撞、占位、
> AOI、个人存档和重登持久化边界。当前权威文件仍为 `Enabled=False`，且
> 导入数据中 `CanTeleportTo=0`，所以真实配置下仍必须安全拒绝，不能人为
> 制造一个可传送目标。视觉、截图、真人听感和设备验收仍暂停，本清单保持
> OPEN。详见 `docs/generated/player-qa/native-ui-controls/R3-AUDIO-WORLDMAP-ZONE-REPORT.md`。

> 2026-08-22 R5 真实 Windows Release 复验：默认安全区蓝柱已消失；Character、
> Inventory、Skill、Quest、Options、Menu、Game Shop、Mail 与 Big Map 均通过
> 可见窗口打开/关闭检查。Inventory 的耐久叠字、Game Shop 的 U+2026 缺字框、
> Quest 皮肤双标题已修复并在重建后的 Release 中复验；Big Map 在约 1-2 秒内
> 收到并显示 map 101、玩家位置和 40 个 NPC。client-bevy native 330/330、
> platform-windows 270/270。后续输入审计已修复 Ctrl/Super 携带 composed text
> 时污染字段及空 text 不回退的问题，client-bevy native 333/333；Windows
> 批量 `type_text` 只落首字符目前更像自动化注入/winit 边界，仍需脱敏事件
> trace 与真人键盘证据区分。Group/Guild 的基础面板已切换到 Crystal 原始
> 几何与素材并完成真实 Release 窗口复验；高级邀请、转让、权限、仓库与非空
> 成员动作仍未收口。登录输入链路与最终 Crystal 逐像素签署未完成。
>
> 2026-08-22 R6：Windows 网关已接收服务端直接 `Chat` 与 `ObjectChat` 两种
> 消息形态，系统/私聊/公会消息不再因只识别 `ObjectChat` 而丢失。`SellItem`、
> `StoreItem`、`TakeBackItem`、`StorageUnlockResult`、`StoragePasswordResult` 与
> `ResizeStorage` 均产生可关联 ACK/NACK；失败结果只结束对应 pending，不在客户端
> 伪造物品或仓库状态。频道过滤与 NPC Sell/Repair 专用服务模式仍为 OPEN。
>
> 2026-08-23 R7 非视觉功能收口：登录/注册已采用独立 in-flight 状态、逐轮去重和
> 脱敏诊断；Storage ACK 只释放一个精确关联操作；NPC Buy/Sell/Repair/
> SpecialRepair 使用互斥的服务端权威模式；聊天已覆盖 Crystal 13 类频道及别名。
> Group 已支持名称输入邀请，Guild 已支持名称招募、公告编辑、成员等级调整、8 个
> 权限位、112 格权威仓库、翻页以及金币存取；Windows 与 Android 均走共享 typed
> intent/command 边界。Registry 为 173。自动化门禁：ui-core 36/36、client-bevy
> default 119/119、native-ui 366/366、Windows 275/275、runtime 180/180、Android
> 45/45，Web typecheck 通过。本轮按用户要求没有控制桌面做视觉验收，因此这里只
> 关闭非视觉代码门禁；真实窗口、真人输入、DPI、Android 真机和逐像素签署仍 OPEN。

## 0. Candidate 硬门禁

- [ ] 所有可见控件均登记在本文件的按钮矩阵中，不存在未登记控件。
- [ ] 所有登记控件均具有明确状态：`Working`、`Verified Original No-op` 或 `Blocked`。
- [ ] 不允许 `Rendered but no handler`、`Wrong panel`、`Placeholder notice` 进入 Candidate。
- [ ] 每个可操作页面至少保留：原版截图、Windows 截图、点击前状态、点击后状态、命令/事件证据。
- [ ] 每个按钮同时通过鼠标点击和适用的键盘路径；不能只用协议脚本证明。
- [ ] 禁用按钮不能发送命令；重复点击、双击和连点不能重复创建、删除、购买或提交。
- [ ] 弹窗打开时，鼠标和键盘不能穿透到世界层。
- [ ] `100% Candidate` 条件：P0=0、P1=0，并且本文件全部必需项打勾。

## 1. 当前代码审计结论

以下状态来自当前 Windows 原生代码，尚不能视为人工验收：

| 页面/区域 | 控件 | 当前状态 | 判定 |
|---|---|---|---|
| 登录 | Account / Password | 可聚焦、逐键输入、退格、Tab；Ctrl/Super 文本污染与空 text 回退已修；自动化批量注入仍需 trace/真人区分 | **OPEN：真人输入与事件边界待验** |
| 登录 | Login | 已连接 `NativeUiIntent::Login`，本地真实登录进入选角 | 成功路径实机通过；错误路径待验 |
| 登录 | New Account | 已连接 `RegisterAccount` | 待完整账号创建页面/结果验收 |
| 登录 | Change Password | 已接真实命令、结果、校验和去重 | 待真实服务端成功/失败验收 |
| 登录 | Safe Key | 已按 Crystal 本地随机软键盘语义实现并导出真实素材 | 待真人鼠标验收 |
| 登录 | Cancel | 退出应用 | 待窗口生命周期验收 |
| 选角 | 角色槽 / Start / New / Delete / Exit | 有处理器 | 待真实鼠标与确认流程验收 |
| 选角 | Credits | 空处理器 | 待对照原版；原版若 no-op 才可接受 |
| 创建角色 | Name / Class / Gender / Create / Cancel | 功能存在，但为通用面板 | **P2 视觉；P1 流程待验** |
| 游戏 HUD | Character | 打开装备窗口 | 真实窗口基础视觉/导航通过 |
| 游戏 HUD | Inventory | 打开背包；图标与格位实机无叠字 | 真实窗口基础视觉/导航通过 |
| 游戏 HUD | Skill | 打开独立 Crystal 技能面板 | 空技能权威状态实机通过 |
| 游戏 HUD | Quest | 独立 QuestLog 状态，三条权威任务，Q 开关 | 真实窗口基础视觉/导航通过 |
| 游戏 HUD | Option | 七项 Options runtime hooks、窗口模式、配置持久化及真实 WAV 音频后端已接线；Crystal 立即提交语义保留 | 代码门禁通过；待真人听感/设备验收 |
| 游戏 HUD | Menu | 打开 Crystal 纵向菜单；Escape 关闭 | 真实窗口基础视觉/导航通过 |
| 游戏 HUD | Group | Crystal 232x249 原始框体、成员视口、Switch/Add/Delete/Close、名称输入邀请与权威状态 | 非视觉功能门禁通过；职业/HP 图标、会长转让和真人输入仍 OPEN |
| 游戏 HUD | Guild | Crystal 590x432 原始框体、Notice/Members/Storage/Ranks、名称招募、公告、成员等级、8 权限位、112 格仓库与金币存取 | 非视觉功能门禁通过；非空真实公会、视觉和真人动作仍 OPEN |
| 游戏 HUD | Game Shop | 105 条权威商品、4x2 分页、选择与禁用购买状态 | 真实窗口基础视觉/导航通过 |
| 游戏 HUD | Mail | 已接严格 Mail 数据状态 | 空邮箱真实窗口通过；非空操作待验 |
| 游戏 HUD | Big Map | map 101、玩家位置、40 NPC 与标记延迟后真实显示 | 真实窗口基础视觉/导航通过；加载态待优化 |
| 游戏 HUD | Minimap Toggle | 独立可见状态 | 待真人点击验收 |
| 聊天栏 | 接收、滚动、频道过滤、Resize、Settings | `Chat`/`ObjectChat` 双形态接收；13 类频道及别名独立过滤；共享状态、Apply/Cancel/Defaults 与本地持久化已接线 | 非视觉代码门禁通过；真人输入仍 OPEN |
| NPC/任务 | 对话选项、Attack、PickUp、Buy/Sell/Repair/SpecialRepair | 已有命令桥；商店服务模式由服务端互斥驱动，按钮按模式 fail-closed | 非视觉代码门禁通过；待 UI 点击与服务端结果验收 |
| 物品弹窗 | Use / Equip / Unequip | 已有命令桥 | 待成功与失败路径验收 |

代码证据入口：

- `apps/game-client/client-bevy/src/native_shell_ui.rs`
- `apps/game-client/client-bevy/src/crystal_ui/login.rs`
- `apps/game-client/client-bevy/src/crystal_ui/select.rs`
- `apps/game-client/client-bevy/src/crystal_ui/hud.rs`
- `apps/game-client/client-bevy/src/crystal_ui/overlays.rs`
- `apps/game-client/client-bevy/src/crystal_ui/chat.rs`
- `apps/game-client/client-bevy/src/quest_ui.rs`
- `apps/game-client/platform-windows/src/shell_bridge.rs`

## Goal UI-0 — 原版页面与按钮清单

所有者：只读 Explorer。不得修改代码。

- [ ] 从 Crystal 客户端逐页记录所有可见控件、点击区域、禁用条件、悬停/按下帧、声音和页面跳转。
- [ ] 记录原版中确实 no-op 的控件；不得根据复刻版注释推断原版行为。
- [ ] 给每个控件分配稳定 ID，例如 `LOGIN.OK`、`SELECT.START`、`HUD.INVENTORY`。
- [ ] 输出 `docs/generated/player-qa/native-ui-controls/original-control-registry.json`。
- [ ] 每个页面保存一张空闲态截图，以及每个弹窗的打开态截图。

验收：注册表中每项均包含 `screen/control/rect/enabledWhen/action/result/closePath/referenceImage`，不存在 `unknown` 必需控件。

## Goal UI-1 — 登录、账号与连接页面

页面：Connecting、Login、Authenticating、错误提示、Connection Lost。

- [ ] Account 输入框：点击聚焦、Tab、Shift+Tab、退格、长度限制和可输入字符与原版一致。
- [ ] Password 输入框：掩码、粘贴策略、清空时机和日志脱敏正确。
- [ ] Login：空字段禁用；错误密码停留登录页；成功进入选角页；一次点击只发一个请求。
- [ ] New Account：走真实账号创建流程；成功、重名/重复账号、非法输入和断线均有正确反馈。
- [ ] Change Password：实现原版页面、旧密码/新密码/确认字段、取消与服务端结果；不得保留 placeholder。
- [ ] Safe Key：先抓原版行为；实现同等页面与服务端契约，或以原版证据确认 no-op。
- [ ] Cancel：干净退出，不留下僵尸进程，不误杀 Gateway。
- [ ] Connection Lost Retry：单击、Enter、Escape 行为一致；密码不会被日志或截图泄漏。

验收：每个按钮进行真实鼠标点击；错误路径和成功路径均有截图、客户端日志与 Gateway 事件证据。

## Goal UI-2 — 角色选择与角色创建

- [ ] 四个角色槽可准确命中；空槽不可选；选中视觉与原版一致。
- [ ] Start：无选择时禁用；有选择时只启动选中角色；双击角色是否启动以原版为准。
- [ ] New Character：达到角色上限时禁用；否则进入原版式创建页面。
- [ ] Delete Character：实现原版确认/取消流程；不能单击后直接不可逆删除。
- [ ] Credits：验证原版行为；原版 no-op 才允许复刻版 no-op。
- [ ] Exit：正常关闭窗口并保存必要状态。
- [ ] 创建角色 Name、Class、Gender：鼠标和键盘均可切换；职业/性别完整，不只覆盖 Warrior/Male。
- [ ] Create：名称校验、重名、非法字符、服务端拒绝、重复点击和成功返回选角页全部正确。
- [ ] Cancel / Escape：返回选角，不创建角色，不污染输入状态。

验收：用一个全新账号完成 `注册 → 登录 → 创建 → 选择 → StartGame → Logout → 删除确认/取消`，全程只通过可见 UI。

## Goal UI-3 — 游戏 HUD 一级按钮

必须逐个完成，不得把多个按钮临时映射到同一个窗口：

- [ ] `Character`：打开角色/装备窗口；再次点击关闭；数据显示与服务端一致。
- [ ] `Inventory`：打开背包；物品格、数量、耐久、悬停和选中正确。
- [ ] `Skill`：打开技能窗口；技能等级、MP、冷却和快捷栏绑定正确。
- [ ] `Quest`：打开任务日志，不得再打开背包；任务状态和目标正确。
- [ ] `Option`：打开设置窗口，不得再打开背包；保存/取消/默认值正确。
- [ ] `Menu`：打开系统菜单；Resume、Character Select/Logout、Exit 等行为以原版为准。
- [ ] `Game Shop`：打开对应页面或按原版展示不可用状态，不能静默 no-op。
- [ ] `Mail`：打开邮件页面；列表、读取、领取、删除和关闭正确。
- [ ] `Big Map`：打开当前地图大图；缩放、关闭和玩家位置正确。
- [ ] `Minimap Toggle`：切换小地图可见性并保留合理状态。
- [ ] `Light Setting`：若原版可操作，循环档位、画面结果和持久化一致。

验收：每个按钮保存 `closed → pressed → opened → closed` 四状态证据；打开错误窗口直接判 P1。

## Goal UI-4 — 游戏内窗口与内部按钮

### 角色与装备

- [ ] 装备槽选择、查看详情、装备、卸下、失败反馈和关闭。
- [ ] 属性数值、职业、等级、经验、负重和装备耐久来自权威快照。

### 背包与物品

- [ ] 物品选中、详情、使用、装备、拆分/合并、丢弃和关闭。
- [ ] 空槽不可触发命令；失败不吞物品；成功立即刷新。

### 技能与快捷栏

- [ ] 技能选中、详情、快捷键绑定/替换/清除。
- [ ] 数字键/F 键与点击快捷栏结果一致；冷却和 MP 不由客户端伪造。

### 任务与 NPC

- [ ] 任务列表选择、详情、跟踪、接受、交付、奖励选择和关闭。
- [ ] NPC 对话链接逐项可点；服务页返回、关闭和输入框流程正确。

### 邮件、商店、仓库和系统页面

- [ ] 邮件读取/领取/删除。
- [ ] NPC Buy/Sell/Repair、数量、确认、取消。
- [ ] 仓库存入/取回/密码/扩容。
- [ ] Option 的音量、显示、快捷键和保存/取消。
- [ ] Menu 的返回游戏、Logout/Character Select、Exit。

验收：成功、禁用和服务端拒绝三类路径都有测试；危险操作具有确认步骤。

## Goal UI-5 — 聊天栏、世界交互与输入优先级

- [ ] Home / Up / Down / End 真正改变聊天滚动位置。
- [ ] All / Shout / Whisper / Lover / Mentor / Group / Guild / Trade 过滤状态与显示内容正确。
- [ ] Resize 和 Settings 有实际行为，不只是入队。
- [ ] Enter 聚焦聊天；Escape 取消；发送后失焦；空消息不发送。
- [ ] 打开任意窗口时，世界点击不会穿透为移动、攻击或 NPC 交互。
- [ ] 拖动窗口、点滚动条和按下按钮时角色不移动。
- [ ] HUD、NPC 对话、死亡弹窗、系统菜单的 Z 顺序和模态优先级正确。

验收：自动测试验证动作队列被消费；真人测试验证鼠标点击不会穿透。

## Goal UI-6 — 窗口、DPI 与状态恢复

- [ ] 1024×768、窗口拖动、最小化/恢复、Alt+Tab 后控件仍可点击。
- [ ] 真机 Windows 100% / 125% / 150% DPI 下点击区域与贴图一致。
- [ ] 调整窗口大小后逻辑舞台缩放一致，不出现黑边点击错位。
- [ ] 断线时当前弹窗安全关闭或恢复；重连后不重复提交旧按钮动作。
- [ ] Logout/Login 后页面状态不串号，密码与敏感信息不保留。

验收：三档真机 DPI 均完成登录、选角、打开/关闭全部一级 HUD 按钮。

## Goal UI-7 — 自动化门禁

- [ ] 为每个按钮建立 `Interaction::Pressed → intent/action → state/packet` 单元或 ECS 测试。
- [ ] 增加“所有 `CrystalHudAction` 必须被显式处理”的穷尽测试。
- [ ] 增加“所有 `CrystalChatAction` 必须有消费结果”的穷尽测试。
- [ ] 增加 Quest 与 Option 不得打开 Inventory 的回归测试。
- [ ] 增加 GameShop/Mail/BigMap/MinimapToggle 非静默 no-op 测试。
- [ ] 增加登录页 Change Password / Safe Key 不得返回 placeholder 的测试。
- [ ] 增加删除角色、购买、丢弃、领取附件的幂等/双击测试。
- [x] 运行 client-bevy、platform-windows、runtime 全测试和 Web typecheck，禁止破坏 Web。

当前自动化门禁证据：ui-core `36/36`、client-bevy default `101/101`、
client-bevy `native-ui` `318/318`、platform-windows `264/264`、runtime `179/179`、
Android `44/44`、Web typecheck 与 component controls `2/2`；registry `141`、
`placeholderCount=0`。`no-op=3` 仅对应 Credits、Light frame visual 和 9 个
Crystal source-disabled menu family。以上不替代真实窗口、DPI、重连或真人验收。

## Goal UI-8 — 原版对照与最终签署

- [ ] 每个页面以相同分辨率、相同状态分别捕获 Crystal 与 Windows。
- [ ] Gemini 视觉模型只评估几何、贴图、字体、状态帧和视觉层；不能代替功能验证。
- [ ] 功能验证必须来自真实点击后的客户端状态与服务端事件。
- [ ] 异模型只读复验按钮矩阵和证据索引。
- [ ] 真人连续操作至少 20 分钟，无死按钮、错页、穿透、卡死或窗口消失。

最终验收：

- [ ] 页面覆盖率 100%。
- [ ] 可见控件覆盖率 100%。
- [ ] 必需控件真实鼠标成功率 100%。
- [ ] P0=0、P1=0。
- [ ] 人工签署 `Accepted`；在此之前只能称为 Internal Candidate。

## 推荐执行顺序

1. UI-0 原版控件注册表。
2. UI-1 登录/账号。
3. UI-2 选角/创建/删除。
4. UI-3 修复 HUD 一级按钮的 no-op 和错页。
5. UI-4 完成内部功能。
6. UI-5 输入优先级与聊天。
7. UI-6 DPI/窗口/恢复。
8. UI-7 自动化门禁。
9. UI-8 同页视觉对照与真人签署。
