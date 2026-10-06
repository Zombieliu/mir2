# Web 对齐 Windows：固定基线目标

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
| W0 | 固定Windows功能清单，逐项核对共享实现、旧入口及差距 | 317条有界记录与9类更正；46个基线blob实际匹配；仍有25条已确认代码差距和未审范围，分母不冻结 | 源码分类与构建证据分开 | 全部待验证 |
| W1 | 支持的桌面路径默认请求共享任务/HUD/角色/技能/邮件/仓库/普通商店与背包，保留显式关闭和 readiness 降级 | 默认请求接线完成；Node 22/22、严格 TSC 通过 | 已纳入 Source09 候选包；Native09 / renderer07 / Next03 / Thin05 build-only 闭合，行为待 W6 | 待验证 |
| W2 | 九类任务动作接入共同 controller / Web host，不新增客户端世界权威 | 九类均有代码候选；Rust Source02 27 distinct 测试和 4 项编译/检查闭合，见本轮状态 | 已纳入 Source09 当前候选包；Native09 / renderer07 / Next03 / Thin05 build-only 闭合，行为待 W6 | 待验证 |
| W3 | 常规 WebGPU / WebGL2 入口选择共享表现，固定首屏 canvas / ABI / package / 单次启动关系 | Node 155/155、严格 TSC、Rust 启动/画布 10/10 通过；可选依赖修复回归通过，三套 WASM 编译检查退出 0 | 已纳入 Source09 当前候选包；renderer07 / Next03 / Thin05 build-only 闭合，运行时与行为待 W6 | 待验证 |
| W4 | 对齐功能、输入、资源及生命周期 | 原七项W4b候选保留；配偶Mail/Whisper完成代码与119项有限检查；后续登录、地图、聊天、物品和商店差距继续排队 | 新增Web代码纳入Next03 / Thin05 | 待验证 |
| W5 | 重建源码对应Core/renderer/Next/独立包并保持预算 | Rust Source09未改；本轮有限119/119与打包诊断16/16；Next03严格TypeScript＋13静态页 | Thin05 7,299文件、372,478,397 B、0链接；原360 MiB cap余5,008,963 B；44 warning静态分类但运行语义未验 | 待验证 |
| W6 | 登录→任务/战斗→保存→重登及平台间行为比较 | 不以夹具替代 | 不以编译替代 | 用户暂缓界面操作 |

W0 审计闭合前不报告整体百分比。此表是执行阶段，不是功能数量或完成率分母。F11 原 44 项行为分母尚未闭合，不冻结。最终功能矩阵将独立列出当前实现、有限检查、当前构建及实际流程证据；两端源码都有功能不能直接勾选“已可玩”。


## 当前状态快照（2026-10-06）

Windows固定基线f72与Web父提交f1cf已细分为317条有界源码记录：103 shared、181 legacy、25 open、8共同限制；不是完整验收分母，不报整体百分比。配偶写信/密语复用既有Mail/Chat入口，新增5项回归和相邻四脚本119/119通过；诊断脚本16/16通过。Next03严格TypeScript＋13静态页、Thin05构建与静态审计退出0，独立包7,299文件 / 372,478,397 B / 0链接，原360 MiB cap余5,008,963 B。Rust Source09未改，限定承接Native09、renderer07和Core原有效输入证据。44条warning已逐条保留并做源码分类，直接导入与部分分支运行语义仍未验。实际服务/HTTP/UI/玩家流程及Pet/Gate远端覆盖未验，goal active。见 [行为矩阵](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/feature-matrix02.md) 与 [QA索引](generated/player-qa/client-core-20260930/web-windows-catchup-20261006/README.md)。

配偶Mail仅开启具名本地草稿，Whisper仅填入Native精确文本`:)`；名字/map、物理owner、scene/map、Bonds epoch/open及完整关系投影在捕获与执行时复核，开Mail后再复核，不自动发送。配偶Online/Offline展示与Whisper可用性共用收到的map。当前四文件独立review02零确认P0/P1；这是源码审查，玩家流程仍待验。

Thin05原样携带Core版本`c4952ff102f1fe1a35cc48b456e68adb33ede425f657d75fa25e6b811e6f979e`及renderer`bevy-e31f4cb651a7b4ef`；全包文件hash核对通过，3,366公共文件与Thin03字节相同，79 atlas PNG相同。231 JSON的原token保持证据只在当前source/output逐对完全相同的条件下限定承接，未新跑parser/lexer。44 warning内容和重复次数与Thin04相同，28源码pin复核相同；编译config仅distDir/distDirRoot变化，不能据此豁免RSC/构建分支/native加载的真实运行检查。

下一轮优先F01共享认证UI/规则：当前Web误把改密raw1当成功而Native成功码为6，注册非8原因被折叠，banned expiry未展示；这些是已有F01行内子问题，不增加/冻结分母。共享Core剩2,605 B预算，需在原cap内实现和测量。当前不允许WASM实例/实际UI或账号请求；计划中的真实WASM Node测试也不执行。历史失败与旧Source/构建回执保留，Thin02超预算旧包不能代替Thin05；原F11 44项分母未恢复、未冻结。

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
