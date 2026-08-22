# Windows 原生 Crystal 视觉复刻执行设计

> 2026-08-22 live current-source revalidation: status is **Visual Validation In
> Progress — not Accepted**. Same-coordinate Crystal/native captures at
> `BichonProvince 288,616` disproved the older “only P2/P3 remains” summary.
> The current-source client first crashed with Bevy `B0001` (overlapping mutable
> minimap queries), then rendered an entirely black world because native
> `standaloneTiles.imageUrl` assets were never queued. Both code-path defects are
> fixed in the working branch. A later same-coordinate pass traced the remaining
> grid-aligned black holes to stable atlas-page keys retaining only the first
> viewport's partial rect layout. Runtime layouts now accumulate rects across
> viewports; live `288,616` evidence reports `657 + 125 = 782` draws,
> `missingBindings=0`, and no black holes. Runtime/native-ui suites pass 169/169
> and 256/256. The same run also fixed Quest Escape fallthrough and replaced Big
> Map's green placeholder with the real MMap image; its outer frame still needs
> Crystal parity. Windows 231/231, keyed-map parity tests, map-source quality
> tests and Web typecheck also pass; the final diagnostic release binary is
> 65,077,760 bytes with SHA-256
> `22BF02AABB42ED34D32B3BE5578B4005CD1824CF5200626150CD33C300A45E8F`.
> Treat the baseline map P0 as closed but Quest, Options, Mail,
> Menu, HUD data/layout and the remaining secondary-panel gaps as P1. See
> `docs/NATIVE-WINDOWS-VISUAL-VALIDATION-REPORT.md`; do not reuse the old 88/100
> HUD score as evidence for the current full scene.

> 2026-08-19 V6 alternate-class/combat update: native entity composition now
> routes Archer `ARArmour`/`ARHair`/`ARWeapon` action families and Assassin
> `AArmour`/`AHair`/directional dual-`AWeapon*` action families with Web-equivalent
> fallbacks. `ObjectStruck` drives pose while authoritative numeric
> `DamageIndicator` events drive bounded animated hit/miss/crit/heal floaters.
> Native F-key target ordering no longer lets a stale selected object mask a
> nearer live hostile. Live evidence records Deer HP
> `10/25 -> 9/25 -> 8/25` and a renderer-owned visible red `1` frame, SHA-256
> `0E24F11B963382F02C82F0DAEEE745F51794A01460175E92523AABE7DCBD49AA`.
> Windows 104/104, runtime 133/133, Simulation 1183/1183, Release SHA-256
> `B6A7078173865DF3415B089DE4119EAA438886EF518AFD9DEC69054B445773D9`
> and Web typecheck pass. Because the starter exports contain zero usable mask
> paths and zero nonzero shadow tuples, the next V6 step is asset exporter/
> pipeline support, followed by spell/projectile effects and lighting; exact
> text, same-scene Gemini review and final human acceptance remain open.

> 2026-08-19 V6 frame-set/composition update: Windows now loads 697 usable
> Crystal per-library catalogs and honors exact action ranges, skips, cadence,
> reverse flags and fallback order. The retained entity path composes body,
> hair, directional front/rear weapon and mount layers, suppresses weapons while
> mounted, and replaces an animation incarnation when body library/mount state
> changes. Native-only overlays render authoritative entity names, dead lines
> and self HP. Live visible proof moved `288,615 -> 287,613`; capture SHA-256 is
> `343B4B05A9E67EF7B687F0DFA9B5D8D2F34E222A7AD95BC03AD6D1E4569E8DC4`.
> Windows 98/98, runtime 133/133, Release and Web typecheck pass. V6 remains
> open for alternate class atlases, shadows/effect masks, combat effects,
> interruption policy, exact text/overlap, lighting and final visual acceptance.

> 2026-08-19 V6 action-clock update: Windows now owns a persistent main-thread
> `AnimationWorld`; packet-authoritative walk/run/attack/range/magic/struck/die/
> revive hints use monotonic sequences, continue changing atlas rects between
> Gateway messages and do not restart on repeated snapshots. A live 254 ms F12
> pair changes 5,087 world-only pixels, while revive/movement remains
> authoritative at `18/18 @ 288,616 -> 288,615`. Windows 90/90, shared runtime
> 133/133, Release and Web typecheck pass. V6 remains open for per-library
> frameSet metadata, stable composite layers, overlays and interruption policy.

> 2026-08-19 world-render execution update: packet-first movement now keeps the
> terrain camera, minimap and HUD on the same authoritative center; schema-v2
> entity atlases route all seven physical pages with Crystal source offsets;
> retained map layouts invalidate when a moved viewport exposes uncached rects;
> and authoritative self health/death overlays drive the native HUD. Live proof
> covers `HP 0/18`, real TownRevive to `288,616`, and field damage to `16/18`.
> Windows 83/83, shared runtime 133/133, Release and Web typecheck pass. Full
> offline asset staging also passes at 8,325 files / 269.91 MiB. Actor actions
> and composite layers, lighting/effects, exact-coordinate full-screen evidence,
> DPI, repository-independent packaged-EXE launch and final human acceptance
> remain open.

> 2026-08-19 HUD Candidate update: the native-only Crystal MainDialog,
> ChatDialog controls/four-line frame, horizontal belt and Bichon minimap are
> implemented and verified from stable authoritative state. Antigravity High
> reviewed the mechanically matched HUD evidence pair as Accepted 88/100 with
> `sameScene=true` and no P0/P1, closing the first HUD Candidate >=85 gate.
> Native UI is 90/90, Windows host is 80/80, Release builds, and Web typecheck
> passes. WN-VIS-006 now moves to map/entity/effect and same-coordinate scene
> convergence; EXP/weight binding, bitmap text, final HUD >=92, DPI/package
> verification and final human acceptance remain open.

状态：2026-08-19 WN-VIS-002 实现已落地但视觉门仍开放，WN-VIS-003/004/005/006 已完成；登录屏与选角屏 AI Accepted 100/100，首个 HUD Candidate Accepted 88/100；地图全覆盖/动画、最终 HUD 92、DPI 与最终人工接受仍开放
交付程序：`mir2-platform-windows.exe`
固定逻辑舞台：1024×768
权威实现：`../Crystal/Client` 代码、Crystal 实机截图和已导出的原始资源
视觉审阅：结构化自动门 + 同场景截图差异 + Gemini 3.7 Flash Medium + 最终人工接受

## 1. Goal

交付一个普通玩家可以在 Windows 上直接启动和游玩的原生客户端：从登录、选角进入比奇，完成任务 1 和任务 2 的移动、战斗、任务物品、背包、交付奖励闭环，退出后重新登录能够恢复；同时让登录、选角和游戏内主 HUD 在 1024×768 基准下达到 Crystal/Mir2 的 1:1 Candidate，并保持 Web 客户端不回归。

本 Goal 的功能闭环已经完成并有六张原生截图和 fresh-account q1→q2 自动证据。后续工作不是再造一套协议或任务系统，而是把独立的 Bevy/Winit/WGPU 表现层从“可操作占位 UI”收敛为 Crystal 表现。

### 1.1 完成定义

只有同时满足以下条件才可把本 Goal 标为完成：

1. Windows EXE 是原生 Bevy/Winit/WGPU 窗口，不是 WebView、Tauri 页面或浏览器套壳。
2. 登录、选角、创建角色、进图、任务 1、任务 2、保存和重登仍由现有 Gateway/Simulation 权威链路驱动。
3. 地图对象不存在黑底、黑三角、错误遮挡或透明色键泄漏。
4. 登录屏使用 Crystal 的登录面板、标题、字段和按钮帧，不再出现通用 620×360 占位卡片。
5. 选角屏使用 Crystal 的专用背景、四个角色槽、角色预览区和底部五按钮栏。
6. 游戏内至少完整呈现 Crystal 1024 布局的底部主框、HP/MP、经验/负重、快捷栏、聊天框、主按钮和右上角小地图。
7. 玩家/NPC/怪物的 body library、动作帧、方向、原点和 y-sort 不再使用错误的通用 fallback。
8. 比奇同场景具有 Crystal 的环境明暗、灯光和安全区可见效果。
9. 100%、125%、150% Windows DPI 下，逻辑舞台、鼠标命中和截图坐标一致，不出现二次缩放模糊或 UI 漂移。
10. Windows、共享 Bevy、WASM WebGL2/WebGPU、Web TypeScript/runtime-policy 和 fresh-account q1→q2 回归门全部通过。
11. 三个固定屏幕的视觉审阅均 `sameScene=true`，无 P0/P1；登录和选角得分至少 90，游戏内 HUD 首次 Candidate 至少 85、最终至少 92。
12. 人工完成一次从登录到任务交付再重登的实际操作，并明确给出 Accepted；AI 分数不能替代这一项。

## 2. 当前基线与证据

### 2.1 已完成的功能层

- 原生登录、注册、角色列表、创建、删除和开始游戏。
- 服务端世界 bootstrap 后才切换 InGame。
- 移动、跑步、转向、NPC 交互、普通攻击、TownRevive、任务交互和快捷背包。
- fresh-account 任务 1 与任务 2，含 20 次服务端确认的稻草人攻击、`GingerTea` 任务物品、30 XP、200 Gold、装备奖励和重登持久化。
- 当前回归基线：Windows 80/80、共享 Bevy 默认 22/22、`native-ui` 72/72、focused map parser 19/19、视觉审阅 harness 11/11、Windows Release 通过；既有 WASM/Web 门保持不回归。

功能证据与人工操作矩阵见：

- `docs/NATIVE-WINDOWS-PLAYABLE-VERTICAL-SLICE.md`
- `docs/NATIVE-WINDOWS-PLAYER-QA.md`
- `docs/generated/player-qa/native-windows-candidate/`

### 2.2 视觉基线

| 屏幕 | Gemini 基线 | Same scene | 主要阻断 |
|---|---:|---:|---|
| 登录 | 100/100（Accepted） | true | WN-VIS-004 已关闭；结构化审阅零可见问题，窗口装饰/debug watermark 属允许差异 |
| 选角 | 100/100（Accepted） | true | WN-VIS-005 已关闭；空角色同场景零问题，另有占用角色动画/StartGame 证据 |
| 游戏内 HUD | 88/100（Accepted） | true | WN-VIS-006 首个 Candidate ≥85 已关闭；权威 EXP/负重、位图名字字体和最终 ≥92 仍开放 |
| 游戏内场景 | 12/100 旧基线 | false | 需用新 HUD 重做同坐标整屏证据；地图/实体动作/灯光和效果仍开放 |

结构化审阅记录：

- `docs/generated/player-qa/ai-visual-review/antigravity-gemini-3.7-native-login-20260819/review.md`
- `docs/generated/player-qa/ai-visual-review/antigravity-gemini-3.7-native-select-20260819/review.md`
- `docs/generated/player-qa/ai-visual-review/antigravity-gemini-3.7-native-game-medium-20260819/review.md`
- `docs/generated/player-qa/ai-visual-review/antigravity-native-login-round1-retry-20260819/review.md`（92/100，发现焦点框与 caret）
- `docs/generated/player-qa/ai-visual-review/antigravity-native-login-final-20260819/review.md`（Accepted 100/100）

Gemini 是视觉缺陷分类器，不是唯一裁判。所有建议必须回到 Crystal 源码、资源元数据和同场景截图验证，不能仅凭模型描述改代码。

### 2.3 本轮实施状态

- `client-bevy/src/crystal_ui/` 已固化登录、四槽选角、MainDialog、ChatDialog、MiniMapDialog、1024×768 舞台和命中规则。
- WN-VIS-002 已生成自包含 native keyed pack：7,149 个引用中输出 4,650 keyed、5 additive；2,494 个缺失源帧安全跳过并作为后续覆盖缺口保留。
- WN-VIS-004 已用 Crystal 登录框、Title 按钮、字段、Tab/Enter、密码掩码和闪烁 caret 替换登录占位布局；Gateway 权威流程不变。
- 首轮审阅 92/100，修正黄色焦点边框和 caret 后，最终审阅 Accepted 100/100。
- WN-VIS-005 已迁移 Crystal 专用选角背景、四槽、选中态、16 帧职业/性别预览、Last Online 行和底部五按钮；空角色审阅 Accepted 100/100，占用角色实窗可见动画并从原生 Start 进入权威比奇。
- WN-VIS-006 已迁移 Crystal MainDialog、HP/MP orb、横向 belt、ChatDialog 控制/四行框、主按钮和 Bichon 小地图；旧固定目标面板已隐藏，任务跟踪改为透明左上角文本。HUD-only 审阅 Accepted 88/100、`sameScene=true`、无 P0/P1。
- V6 已接入 697 个逐库 Crystal frameSet，并实现基础 body/hair/前后武器/mount 复合、骑乘武器抑制、原生名字/死亡行与自体血条；实窗移动证据证明图层和名牌跟随权威坐标。
- 当前自动门：Windows 98/98、共享 runtime 133/133、Release build 和 Web typecheck 通过；登录/选角既有资源与发布门保持不变。
- 下一单写者任务为 alternate class library、阴影/effect mask、战斗特效与灯光，再处理精确文字/重叠策略和同坐标整屏收敛；权威 EXP/负重仍作为 HUD P2/P3 backlog 保留。

## 3. 已确认的差距根因

### 3.1 Web 与 Windows 为什么差距大

两端共享的是后端语义、协议、read model、部分 Bevy 世界渲染和资源；最终 UI 并不共享：

- Web 的完整外壳由 React/DOM/CSS、浏览器图片处理和 Web 专用组合层完成。
- Windows 的最终外壳必须由 Bevy UI/Sprite/WGPU 单独实现。
- Gateway/Simulation 只回答“角色有什么状态、任务发生了什么”，不会提供按钮坐标、HUD 皮肤、字体或透明色键处理。

因此“跨平台代码已经合并”只证明原生宿主和共享 runtime 能运行，不等于 Web UI 自动变成 Windows 原生 UI。

### 3.2 地图黑块不是 GPU 性能问题

Web 地图实际使用“安全 atlas + standalone 对象/加法帧”双通道。`buildMapTileDrawList` 会把 `mapAtlasPathRequiresAlphaKey` 或 additive 的帧排除出普通 atlas；standalone decode 再通过 `apps/web/lib/scene-alpha-key.ts` 对 Mir2 旧地图对象执行：

1. 从边缘 flood-fill 去除近黑色背景；
2. 按亮度羽化边缘；
3. 把 1-bit 棋盘阴影重建为统一半透明阴影。

Windows 的 `apps/game-client/platform-windows/src/map_parser.rs` 当前只有 atlas 通道：只要 rect 能命中就写入 `tiles`，输出中的 `standaloneTiles` 和 `retainedImageKeys` 固定为空，而且已经解析出的 additive 位没有进入最终绘制状态。当前提交的 map-atlas manifest 只包含 Tiles/SmTiles 等 raw-safe 库，因此 Objects 等帧在 Windows 要么缺失；若使用旧/扩展 manifest 命中未经抠色的对象页，则会直接暴露 opaque matte。共享 runtime 已经有 standalone/additive 绘制分支，缺的是 Windows producer 的路由和原生图片供给。

这解释了 Windows 与 Web 在对象覆盖、透明边缘、阴影和发光帧上的系统性差异，也与截图中的黑三角/黑块区域一致。第一修复必须恢复 Web 已验证的双通道语义，不能用隐藏整个对象层、统一加透明度或换背景色掩盖。

### 3.3 登录与选角是占位布局

`apps/game-client/client-bevy/src/native_shell_ui.rs` 当前固定创建：

- 1024×768 根节点；
- `ChrSel/0.png` 全屏背景；
- 620×360 半透明通用面板；
- 默认 Bevy 字体和纯色按钮。

这解释了登录仍能看到正确开场石纹，但中心框、按钮和文字不一致；也解释了选角继续错误复用登录背景。

### 3.4 游戏内 HUD 是功能面板，不是 Crystal HUD

`apps/game-client/client-bevy/src/quest_ui.rs` 当前用多个半透明矩形展示玩家属性、任务、NPC 对话、目标、拾取、控制提示和背包。数据是权威的，但布局、皮肤和层级不是 Crystal 的 `MainDialog`、`ChatDialog`、`SkillBarDialog` 和 `MiniMapDialog`。

### 3.5 实体图集只有 body 起步层

`apps/game-client/platform-windows/src/atlas.rs` 当前根据 `kind` 选择一个默认 body library，并把每个对象建成一个 `:body` layer。它尚未完整覆盖：

- 角色身体、头发、武器、装备的多层组合；
- stand/walk/run/attack/struck/die 等动作范围；
- Crystal 帧 `x/y` 原点偏移；
- 每类 NPC/怪物的真实 body library；
- 影子、特效和动作结束语义。

因此透明色键修复完成后，仍要单独收敛实体帧、原点和分层。

## 4. 设计原则与代码边界

### 4.1 权威状态与表现严格分离

保留现有 `NativeShellModel`、`UiReadModel`、任务模型和 Gateway 命令语义。视觉组件只能：

- 读取模型；
- 显示模型；
- 产生已定义的玩家意图。

视觉代码不得直接增加 XP/Gold、完成任务、生成掉落、修改服务端坐标或伪造角色。

### 4.2 Web 不与 Windows 共用最终 UI

共享范围：

- `apps/web/public/original-ui` 和地图/实体图集；
- Crystal 帧索引、尺寸和原点元数据；
- Gateway/Simulation/read model；
- 可复用的纯 Rust 数据选择、动作状态和坐标计算。

独立范围：

- Web：React/DOM/CSS；
- Windows：Bevy UI、Sprite、Winit、WGPU、Windows DPI/input。

默认不修改 Web UI。共享资产变化必须是添加式或内容哈希版本化；若改变现有图集字节，必须同时执行 Web 视觉回归。

### 4.3 1024×768 逻辑舞台

所有 Crystal 坐标在一个固定 1024×768 逻辑空间表达：

- 原生窗口初始 client area 为 1024×768；
- DPI 变化只影响 OS 物理像素和窗口缩放，不改变逻辑坐标；
- 若以后允许任意窗口尺寸，使用整数优先的 uniform scale 和 letterbox；
- UI 布局与鼠标命中使用同一 `StageTransform`；
- 像素资源默认 nearest sampler，不做 CSS 风格自由拉伸；
- 只有明确的全屏背景和可拉伸区域允许缩放。

建议增加平台无关的纯数据结构：

```rust
pub struct CrystalStageMetrics {
    pub logical_width: f32,  // 1024
    pub logical_height: f32, // 768
    pub scale: f32,
    pub offset: Vec2,
}

pub struct CrystalFrameSpec {
    pub library: &'static str,
    pub index: i32,
    pub width: f32,
    pub height: f32,
    pub offset_x: f32,
    pub offset_y: f32,
}
```

### 4.4 UI 模块化目标

不要继续把所有布局堆进 `native_shell_ui.rs` 或 `quest_ui.rs`。目标结构：

```text
apps/game-client/client-bevy/src/crystal_ui/
  mod.rs             plugin、screen routing、公共 resources
  metrics.rs         1024×768 舞台和 DPI/letterbox 变换
  assets.rs          Crystal library/index -> 路径/尺寸/原点
  widget.rs          image button、field、label、orb、bar 公共构件
  login.rs           Login/Connecting/Authenticating/Error
  character_select.rs
  character_create.rs
  hud.rs             MainDialog 主框、HP/MP、经验、负重、主按钮
  chat.rs            ChatDialog 和输入/滚动
  belt.rs            快捷栏/技能栏
  minimap.rs         MiniMapDialog 和地图裁剪/标记
  overlays.rs        NPC/任务/目标/掉落等非主框浮层
```

迁移期间：

- `native_shell_ui.rs` 只保留兼容 plugin 导出和状态路由，或在迁移结束后成为薄 wrapper；
- `quest_ui.rs` 保留意图和功能模型，不再拥有最终主 HUD 皮肤；
- 每轮最多一个 Agent 修改 `native_shell_ui.rs`，最多一个 Agent 修改 `quest_ui.rs`；
- `apps/game-client/runtime/src/lib.rs` 属于高冲突共享 runtime，一轮只能有一个明确 writer。

## 5. Crystal UI 规格表

以下数据来自当前 `../Crystal/Client` 源码和 `apps/web/public/original-ui/*/meta.json`，是实现基准，不是截图估算。

### 5.1 登录屏

背景：

- `ChrSel` 从 index 0 开始，19 帧，100 ms；登录框在开场完成后显示。
- 最低 Candidate 可固定使用最终稳定背景帧；最终接受应恢复 Crystal 的开场时序。

登录框：

| 元素 | 资源 | 尺寸/位置 |
|---|---|---|
| 对话框 | `Prguse/1084.png` | 328×220，屏幕居中 `(348,274)` |
| LOG IN | `Title/30.png` | 102×24，框内水平居中，y=12 |
| ID | `Title/31.png` | 32×20，框内 `(52,83)` |
| PASS | `Title/32.png` | 32×20，框内 `(43,105)` |
| Account field | 原生文本输入 | 框内 `(85,85)`，136×15 |
| Password field | 原生密码输入 | 框内 `(85,108)`，136×15 |
| OK | `Title/320..322` | 素材 48×48、控件 42×42，框内 `(227,81)`；normal/hover/pressed |
| NEW | `Title/323..325` | 100×25，框内 `(60,163)` |
| CHANGE PASS | `Title/326..328` | 100×25，框内 `(166,163)` |
| SAFE | `Title/332..334` | 100×25，框内 `(60,189)` |
| CANCEL | `Title/329..331` | 100×25，框内 `(166,189)` |

第一阶段可以让 CHANGE PASS 和 SAFE 显示为不可用但保留原版视觉；不得把它们删除导致构图错误。CANCEL 在 Windows 原生应关闭窗口或按产品策略回退，不能误发网络命令。

### 5.2 选角屏

| 元素 | 资源/规则 | 位置 |
|---|---|---|
| 背景 | `Prguse/65.png`，1024×768 | `(0,0)` |
| SELECT 标题 | `Title/40.png`，84×19 | `(468,20)` |
| 服务器名 | 文本，155×17 居中 | `(432,60)` |
| 角色预览 | `ChrSel/220` 起，16 帧/250 ms，`UseOffset` | `(260,420)` |
| 预览叠层 | 当前角色显示帧 + 560 | 与角色预览同原点 |
| 槽 1 | `Prguse/44` 空；有角色用 `Title/660+class` | `(637,194)` |
| 槽 2 | 同上 | `(637,298)` |
| 槽 3 | 同上 | `(637,402)` |
| 槽 4 | 同上 | `(637,506)` |
| 选中态 | 有角色 index 再加 5 | 每槽不变 |
| 名字 | 槽内 `(107,9)`，170×18 | 随槽 |
| 等级 | 槽内 `(107,28)`，30×18 | 随槽 |
| 职业 | 槽内 `(178,28)`，100×18 | 随槽 |

1024 宽时，Crystal 的底部五按钮按 `xPoint=(1024-200)/5` 计算：

| 按钮 | normal/hover/pressed | 屏幕位置 |
|---|---|---|
| START | `Title/340..342` | `(132,736)` |
| NEW CHAR | `Title/343..345` | `(296,736)` |
| ERASE CHAR | `Title/346..348` | `(460,736)` |
| CREDITS | `Title/349..351` | `(624,736)` |
| EXIT | `Title/352..354` | `(788,736)` |

每个底部按钮资源为 100×25。角色槽资源为 288×54/56。空槽仍必须占位，不能根据角色数缩短列表。

### 5.3 游戏内主框

Crystal 1024 布局的基础资源与定位：

| 元素 | 资源/尺寸 | 位置 |
|---|---|---|
| MainDialog 背板 | `Prguse/1.png`，1024×152 | `(0,616)` |
| ChatDialog | `Prguse/2221.png`，632×68 | MainDialog x+230，y=671 |
| MiniMapDialog | `Prguse/2090.png`，128×154 | `(898,0)`；Crystal 用 `ScreenWidth-126`，右侧有 2 px 越界/裁切语义 |
| 角色按钮 | `Prguse/1900..1902`，20×20 | MainDialog `(905,76)` |
| 背包按钮 | `Prguse/1903..1905`，20×20 | MainDialog `(928,76)` |
| 技能按钮 | `Prguse/1906..1908`，20×20 | MainDialog `(951,76)` |
| 任务按钮 | `Prguse/1909..1911`，20×20 | MainDialog `(974,76)` |
| 选项按钮 | `Prguse/1912..1914`，20×20 | MainDialog `(997,76)` |
| 菜单按钮 | `Prguse/1960..1962`，40×40 | MainDialog `(969,35)` |
| 商城按钮 | `Prguse/826..828`，40×38 | MainDialog `(919,35)` |
| 小地图切换 | `Prguse/2102..2104`，16×15 | MiniMap `(109,3)` |
| 大地图 | `Prguse/2096..2098`，20×20 | MiniMap `(25,131)` |
| 邮件 | `Prguse/2099..2101`，20×20 | MiniMap `(4,131)` |
| 光照状态 | `Prguse/2093.png`，20×20 | MiniMap `(102,131)` |

首个 HUD Candidate 必须先恢复主框、聊天框和小地图外框，再把现有权威数值放入对应区域。任务追踪、NPC 对话和目标 HP 可以暂时作为 Crystal 风格独立窗口，但不能遮挡主框、小地图或角色视野中心。

聊天框首轮使用 `Prguse/2221` 的四行默认态；后续展开态使用 `2224`（7 条内容行，632×116）和 `2227`（11 条内容行，632×164），不能用自由拉伸同一张图片替代。

## 6. 地图透明色键修复设计

### 6.1 方案比较

| 方案 | 做法 | 优点 | 风险 |
|---|---|---|---|
| A：离线 keyed standalone 资源（推荐） | 仅对 Bichon/交付地图实际引用的对象源 PNG 单独执行黑底 flood-fill/羽化/阴影重建，输出内容寻址 PNG；Windows 按 Web 规则把对象/additive 帧路由到 standalone | 最接近 Web 已验证语义；运行时稳定；不改现有 Web atlas 字节 | 增加派生资源和构建门，需管理缺帧与发布体积 |
| B：Windows 运行时解码 standalone PNG | producer 路由到 raw standalone，原生端解码每张图片并执行等价算法后上传 | 不生成第二套长期资源 | 启动 CPU、缓存、异步 ready 和色彩空间风险更高 |
| C：启动时处理整个 atlas 页 | 解码 atlas 页并逐 rect 抠色后上传 | 页面数量少 | 当前安全 atlas 不含普通 Objects；需重做 atlas 内容且容易误处理 additive/地板，不推荐 |

推荐 A：新增 native-only、内容寻址的 keyed object 输出，不覆盖 `generated/map-atlas` 或 `original-map`；Windows producer 复刻 Web 的 atlas/standalone/additive 分流，共享 runtime 只增加可选 standalone `imageUrl` 加载。这样没有 native 字段时，WASM 行为保持不变，也不把逐像素处理放进玩家启动路径。

### 6.2 A 方案的精确边界

建议新增：

- `apps/web/scripts/build-native-keyed-map-pack.mjs`（或独立 `tools/native-map-assets/`）
  - 从 map `0` 及明确交付地图枚举实际对象帧，不盲目转换全资源库；
  - 复用或逐 fixture 对齐 Web `alphaKeyMapObjectPixels`；
  - 常量保持 `SOLID=18`、`FEATHER=72`、shadow luma 48、shadow alpha 120；
  - additive 帧不做普通黑键，使用已有 `generated/original-map-blend` 语义；
  - 输出 `/generated/native-map-keyed/<library>/<frame>.<hash>.png` 和 manifest。
- 路由策略 fixture
  - 覆盖 `Objects*`、`SmObjects*`、`Dungeonsc`、`Wallsc` 等 requires-alpha-key 库；
  - 覆盖 Tiles/SmTiles raw-safe 库；
  - 覆盖 middle/front additive 位。

受控修改：

- `apps/game-client/runtime/src/lib.rs`
  - `MapStandaloneTile` 增加可选 `imageUrl`；
  - 在 standalone ready 检查前通过 AssetServer 加载该 URL，并纳入现有 image retain/release；
  - `imageUrl` 缺失时完全保留当前 Web 像素上传合同。
- `apps/game-client/platform-windows/src/map_parser.rs`
  - 复刻 Web `requiresAlphaKey` 库分类；
  - raw-safe floor -> `tiles` atlas；
  - keyed object -> `standaloneTiles` + native keyed `imageUrl`；
  - additive -> `standaloneTiles(additive=true)` + blend 资源 URL；
  - 保留 Crystal drawMode、frame offset、bottom anchor、z 和动画帧语义。

`gateway.rs` 继续只推 MapRenderState，不是本轮 writer。`native_ingest.rs` 也不需要增加大图消息。该批不得同时重写 HUD 或实体动画，便于截图确认地图双通道闭环。

### 6.3 必需测试

1. 离线 packer 与 Web 固定 5×5/7×7 RGBA fixture 的输出逐字节一致。
2. border-connected 黑色被清除，内部黑色线条被保留。
3. feather 区 alpha 单调且范围正确。
4. 棋盘阴影变为 alpha 120，普通实心暗色对象不被误判。
5. Tiles/SmTiles 地板库仍进入 atlas，不生成 keyed standalone。
6. Objects/SmObjects/Dungeonsc/Wallsc 进入 standalone，且 keyed manifest 缺帧时构建失败而不是运行时加载 raw 黑底。
7. middle/front additive 位进入 `standaloneTiles(additive=true)`，不做普通 alpha-key。
8. 带 `imageUrl` 的 standalone 资源在 Bevy ready 后切换且保持旧帧直至加载完成；无 `imageUrl` 的 Web payload 行为不变。
9. 比奇固定坐标截图的纯黑异常像素统计降为零或与 Crystal 容差一致，视口移动时没有一帧黑块闪烁。

## 7. 实施批次

### V0：确定性验收夹具

输出：

- 固定账号/角色或可重复 seed；
- 登录、选角、比奇 `0 @ 335,262` 三个 capture target；
- 统一 1024×768 PNG；
- 每张截图记录 git revision、EXE hash、asset manifest hash、DPI、坐标、方向、light、截图时间；
- AI review 命令不允许模型执行 shell，只传参考图、候选图和 rubric。

退出门：同一次 smoke 能稳定捕获三屏，游戏截图 `sameScene=true` 的前置状态可重复。

### V1：地图对象透明与阴影

执行第 6 节 A 方案，先恢复 Web 已验证的 atlas/standalone/additive 双通道并修 P0 黑三角/黑块。

退出门：

- 固定比奇截图不再出现纯黑对象背景和三角块；
- 地板、树、墙、灯柱前后层和人物 y-sort 正常；
- Windows runtime + Web 两后端回归通过。

### V2：Crystal UI 基础设施

新增 `crystal_ui/assets.rs`、`metrics.rs`、`widget.rs`：

- 帧路径、尺寸、原点的类型化注册；
- normal/hover/pressed/disabled image button；
- 绝对定位 image control；
- 文本输入、密码遮罩、焦点和 caret；
- 逻辑舞台/DPI 转换；
- GDI 文字图片优先、动态文字 outline fallback。

退出门：结构单测、hover/pressed 命中测试、DPI 坐标 round-trip 和无资源 404。

### V3：登录屏

用第 5.1 节规格替换通用面板，保留现有 `NativeShellModel` 和 Gateway 意图。

退出门：

- 对话框、字段和五按钮的资源/坐标断言通过；
- 键盘 Tab/Enter、鼠标、密码遮罩和错误提示仍可用；
- Gemini `sameScene=true`、无 P0/P1、≥90；
- fresh-account 登录仍通过。

### V4：选角与建角

用第 5.2 节规格建立独立选角 scene；角色槽永远四个，使用服务端角色数组填充。建角对话框按 Crystal 源码单独复刻，不在选角主屏上继续堆表单。

退出门：

- 背景 `Prguse/65`、四槽、选中态、预览和底部五按钮全部存在；
- create/delete/start 使用真实 `character_index`；
- 空槽、1 个角色和 4 个角色快照测试；
- Gemini `sameScene=true`、无 P0/P1、≥90。

### V5：游戏内主 HUD、聊天、快捷栏和小地图

顺序：

1. `MainDialog` 背板与层级；
2. HP/MP orb、等级、经验、负重、金币和角色名；
3. ChatDialog 背板、四行历史、输入和滚动；
4. belt/快捷栏和 key labels；
5. 主按钮 hover/pressed 和窗口 toggle；
6. MiniMapDialog 外框、地图裁剪、玩家/NPC/任务标记；
7. 把现有 quest/NPC/target/pickup 功能窗迁到不冲突的 Crystal 窗口。

退出门：主 HUD 和小地图资产/坐标门通过；鼠标不穿透 UI 触发世界移动；任务 1→2 全流程仍可操作；首个游戏视觉 Candidate ≥85。

### V6：实体动作、分层和原点

该阶段只补 Windows 的“实体表现生产端”，继续复用共享 Bevy 的
`EntityRenderLayer`/稳定实体同步，不重写共享 renderer。单写者按以下顺序执行：

1. 在原生 adapter 中把 Walk/Run/Attack/Struck/Die/Revive 包转换为
   `action + monotonic token + started_at_ms`；新增 Windows 每帧动画时钟，
   即使 500 ms 没有新网络包也必须继续换帧；
2. 把各实体库 `meta.json` 的 `frameSet` 作为可选字段写入 schema-v2
   manifest，按 library 解析 action 的 start/count/skip/interval/reverse，
   不能给全部怪物强套同一默认帧表；
3. 最终帧统一经过共享 `AnimationPose::draw_frame_index`，覆盖八方向、
   负 skip、reverse、Die 结束固定 Dead，以及 NPC 只支持的动作子集；
4. 由单层 `body` 扩展为稳定 key 的 `shadow -> mount -> rearWeapon -> body
   -> hair -> frontWeapon`，同一动作相位驱动所有层；武器前后关系照搬
   Crystal `PlayerObject.Draw` 和 Web 已验证的方向表；
5. 新增仅由 Windows host 注册的 nameplate/HP-bar overlay；名字跟随实际
   frame bounds，死亡不显示血条，NPC 默认无战斗血条；
6. 首个覆盖集只做男女玩家、一个 NPC、默认怪物和一个特殊 frameSet
   怪物；通过后再按比奇 MVP 实际 shape 扩充装备页，禁止一次性常驻全库。

推荐新增 `platform-windows/src/entity_presentation.rs` 和
`entity_overlays.rs`。只有公开纯动画类型确有必要时才最小修改 shared
runtime；不得在此批改变共享 system chain、z/opacity 或 Web 的层语义。

退出门：固定玩家、Jane/Jude、Scarecrow、Deer 的各方向/动作金图；
stand/walk/run/attack/struck/die/dead/revive 状态测试；跨页/缺帧 fallback；
无脚底漂移、无动作重启、无层级穿插；触及 shared runtime 或 manifest
时 Web 类型检查、帧表测试与 Bevy runtime 全测试必须同时通过。

### V7：光照、安全区和环境效果

复用 Web 已验证的 Crystal light/weather/effect 语义，Windows 用 WGPU/Bevy material 实现：

- ambient light；
- lamp radial light；
- 安全区边界/粒子；
- additive effect blending；
- day/night/map override。

退出门：固定 light=1 和 Day 两组同场景截图，无黑 alpha 写入；游戏最终视觉分 ≥92。

### V8：DPI、性能、回归和人工接受

矩阵：

- Windows 缩放 100%/96 DPI、125%/120 DPI、150%/144 DPI；固定逻辑舞台
  1024×768 时，预期物理客户区分别为 1024×768、1280×960、1536×1152；
- 每档验证窗口物理尺寸、hover/pressed/点击命中、F12 原始物理截图，另产出
  canonical 1024×768 PNG；记录 DPI、缩放算法、EXE/资源哈希；
- 运行中跨显示器改变 DPI 后重复尺寸、鼠标命中和截图门；
- Debug 与 Release；
- NVIDIA/集显（可用硬件范围内）；
- 窗口首次启动、Alt-Tab、最小化恢复、断线重连、关闭保存；
- 登录→选角→任务 1→任务 2→退出→重登。

发布包必须从全新目录生成 `mir2-platform-windows.exe + mir2-assets/`，清除
`MIR2_NATIVE_ASSET_ROOT`，从仓库外工作目录启动。当前 8,325 文件 / 269.91
MiB 的本机资源 staging 已通过，但以下仍是 P0：native-keyed 和新增 ChrSel
帧必须能从 clean checkout/CI 生成；资源根必须验证完整 sentinel 集而不是任一
manifest 即接受；缺包的 Release 必须明确失败，不能打开缺图窗口；CI 必须真正
执行仓库外启动 smoke，而不只是 `Get-Item`。

每档证据至少包含：`dpi.json`、`window-state.json`、`mouse-hit.json`、
`raw-window.png`、`canonical-1024x768.png`、`exe.sha256`、
`asset-manifest.sha256`、`stdout.log` 和 `stderr.log`。

性能基线在同一硬件记录启动时间、首图时间、显存/内存、平均帧时间和 p95，不以降低图像质量换取通过。

## 8. Agent 编排和模型分工

### 8.1 角色

| 角色 | 推荐模型/强度 | 职责 |
|---|---|---|
| 总设计与集成 | frontier reasoning，high/xhigh | 架构、跨模块边界、冲突处理、最终 review、接受结论 |
| 有界代码 worker | `gpt-5.3-codex-spark`，high | 单文件/单模块实现、测试、机械迁移；每次必须给精确写集 |
| 探索/规格 worker | Spark medium 或可用轻量代码模型 | 只读 Crystal 源码、资源索引、测试缺口，不修改文件 |
| 视觉 reviewer | Gemini 3.7 Flash，Medium | 每个里程碑三屏结构化差异；默认 Medium，不是每次 High |
| 最终视觉复核 | Gemini High 或第二视觉模型 | 只用于 Medium 结论不稳定、最终 Candidate 或细粒度争议 |
| 备用实现/独立 review | Qwen/DeepSeek 可用代码模型 | 有界测试、资源表、独立 diff review；不得独立决定架构或直接合并 |

如果当前子 Agent 接口未暴露 Spark，不应伪称使用 Spark；可用轻量 worker 只做只读审计或暂停该批，主 Agent 继续关键路径。外部模型输出一律视为未信任建议，必须由主 Agent 阅读 diff、运行测试并核对源码。

### 8.2 每轮模板

每一轮必须声明：

1. 目标和退出门；
2. writer 名称、模型和精确可写文件；
3. high-conflict 文件唯一 owner；
4. explorer 的只读范围；
5. 自动测试命令；
6. 要捕获的截图；
7. Web 回归门；
8. 失败时回退到哪个已知基线。

禁止两个 worker 同时修改：

- `apps/game-client/runtime/src/lib.rs`
- `apps/game-client/client-bevy/src/native_shell_ui.rs`
- `apps/game-client/client-bevy/src/quest_ui.rs`
- `apps/game-client/platform-windows/src/map_parser.rs`
- `apps/game-client/platform-windows/src/atlas.rs`
- `apps/game-client/platform-windows/src/main.rs`

## 9. 第一轮可执行任务

### WN-VIS-001：固定三屏 capture manifest

写集：`apps/game-client/platform-windows/src/capture.rs`、`apps/game-client/platform-windows/scripts/`、新 evidence manifest。
禁止修改 renderer。
输出：可重复登录、选角、`0 @ 335,262` 截图及环境元数据。

### WN-VIS-002：Windows map alpha-key parity

状态：双通道路由实现完成并通过自动门；因缺失源帧覆盖、地图动画和同场景证据未关闭，本任务的最终视觉退出门仍开放。

唯一 shared-runtime writer。写集限于第 6.2 节文件。
输出：native keyed object pack、路由 fixture、standalone URL 加载、固定比奇截图。
退出门：P0 黑块关闭，所有 map/runtime tests 通过。

### WN-VIS-003：Crystal UI asset/metrics 基础

状态：完成；登录/选角/HUD 规格、1024 舞台和 widget 基础已进入共享 Bevy 层。

与 WN-VIS-002 并行，但只能新增 `client-bevy/src/crystal_ui/{assets,metrics,widget}.rs` 和对应测试；不得修改 `native_shell_ui.rs`、`quest_ui.rs`、runtime。
输出：第 5 节帧注册、1024 舞台、按钮状态和 DPI 纯数据测试。

### WN-VIS-004：登录屏迁移

状态：完成；最终结构化审阅 Accepted 100/100，`sameScene=true`，零可见问题。

依赖 WN-VIS-003。该轮唯一 `native_shell_ui.rs` writer。
输出：Crystal 登录布局和截图；保持现有 Gateway 状态机不变。

### WN-VIS-005：选角屏迁移

状态：完成；空角色同场景结构化审阅 Accepted 100/100，`sameScene=true`，零可见问题；占用角色动画和原生 StartGame 已另行实窗验证。

依赖 WN-VIS-004。继续由同一 shell owner 或在清晰 handoff 后换 worker。
输出：专用背景、四槽、16 帧 source-offset 预览、Last Online、底部五按钮、空/占用角色截图门和可复用空角色夹具。

### WN-VIS-006：游戏内主 HUD Candidate

状态：首个 Candidate 完成；HUD-only Antigravity High 审阅 Accepted 88/100，`sameScene=true`，无 P0/P1，达到规划的 ≥85 退出门。

写集：`client-bevy/src/crystal_ui/{hud,chat,minimap,spec}.rs`、Windows 插件注册和仅原生加载的任务表现层。
输出：Crystal MainDialog、底部裁剪 HP/MP orb、横向 belt、ChatDialog 控制/四行框、主按钮、Bichon `MMap/101` 裁剪和权威实体标记；稳定截图、机械 HUD evidence pair 和结构化审阅报告。
保留项：EXP/负重权威字段、位图名字字体、同坐标整屏场景、最终 HUD ≥92 和人工接受。

## 10. 验证矩阵

### 10.1 每个代码批次

- Rust fmt 和 diff check。
- 受影响 crate 单测。
- Windows native debug build。
- `client-bevy` default 与 `native-ui` feature tests。
- 若触及 shared runtime：WASM WebGL2 和 WebGPU build。
- 若触及 shared assets/manifest：Web TypeScript、asset policy、对应 Web 截图门。

### 10.2 每个视觉批次

- 固定 1024×768 screenshot。
- 结构断言：必须资源、坐标、尺寸、z-order、点击区域。
- 像素统计：纯黑异常、透明边缘、区域 MAE/changed pixels。
- Gemini Medium 结构化 JSON/Markdown review。
- P0/P1 为零后才进入下一屏；P2 进入显式 backlog。

### 10.3 最终门

- fresh-account q1→q2 自动 smoke。
- 手工鼠标/键盘完整流程。
- 保存与重登。
- 100/125/150% DPI。
- 三屏 Gemini 最终复核。
- 人工 Accepted。

## 11. Web 影响规则

默认路径对 Web 无影响，因为 UI 代码位于 `client-bevy` 原生 feature 和 `platform-windows`。以下情况会影响 Web，必须额外验证：

- 修改 `apps/game-client/runtime/src/lib.rs` 的共享系统顺序、材质或 atlas 绑定；
- 修改 `apps/web/public` 下已有资源字节或 manifest；
- 修改 shared read model/schema；
- 修改 WASM feature/default plugin；
- 把 Windows DPI/窗口假设写入共享 stage 逻辑。

降低风险的方法：

- native inbound 新变体默认无消息时保持 WASM 字节行为不变；
- Windows UI plugin 只由 `platform-windows/main.rs` 注册；
- 共享资源只新增版本化文件，不覆盖 Web 当前 release；
- 所有 shared runtime 改动必须有“无 native 消息时等价”测试。

## 12. 停止条件

仅在以下情况暂停并请求用户决定：

- Crystal 源码与已接受截图对同一界面给出冲突规格，且无法通过当前原版程序复现；
- 需要替换或重新发布大体积私有资产包；
- 需要修改账号、认证、安全或生产数据迁移；
- 视觉取舍会改变产品范围，例如改成现代化 UI 而非 1:1；
- 最终人工 Accepted 需要用户亲自判断。

普通编译错误、缺测试、模型审阅失败、截图分数低和实现难度不是停止条件。
