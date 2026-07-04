# OriginalClientShell 渲染宿主 — client map

> 客户端「前置铺垫」文档之一。索引与「如何加功能」配方见 apps/web/CLAUDE.md。

## 这块是干什么的 (What it does)

`OriginalClientShell` 是整个玩家 UI 的**渲染宿主 (render host)**：它接收 page.tsx 算好的所有
state（通过 props），决定当前画哪个**屏幕**（login / select / game），并组合出**游戏视口
(viewport)**——Bevy canvas + DOM 地图/实体层 + 场景覆盖层——以及 HUD、背包、角色面板。
它是 page.tsx 主 JSX `return` 里挂的第一个组件（page.tsx:11107），lazily mounted
（`dynamic(..., { ssr:false })` at page.tsx:140-141，因为 Bevy WASM 要等 `#mir2-web3-canvas` DOM 存在才能 boot）。

The shell is **presentation-only**: 它不持有 WebSocket、不调用 `send`、不写 `world`。所有玩家动作
都经过 `on*` 回调 prop 冒泡回 page.tsx 的 action handler。它和 `<ExtraWindows>` 是 page.tsx 的
**两个并列渲染面**：shell = 核心三屏 + 视口 + 背包/角色；ExtraWindows = stage5 社交/工具窗口。

确认：文件在 `apps/web/app/original-client-shell.tsx`（**`app/` 下，不是 `app/components/`**），3527 行。

## 入口在哪 (Entry points)

| 文件 File | 作用 Role | 关键符号 Key symbols (file:line) |
|---|---|---|
| `apps/web/app/original-client-shell.tsx` | 渲染宿主组件本体 | `OriginalClientShell({...props})` :337 · 主 JSX `return (` :2019 |
| `apps/web/app/original-client-shell.tsx` | 三屏路由 (ternary on `screen`) | login overlay :2048/:2189 · `<SelectOverlay>` :2215 · `<GameUiScene>` :2231 |
| `apps/web/app/original-client-shell.tsx` | 视口骨架 (stage frame) | `.client-stage-frame` `<div>` :2022 · `<canvas id="mir2-web3-canvas">` :2069 · `<GameSceneBackdrop>` :2087 |
| `apps/web/app/components/original-client-shell-types.ts` | **完整 props 类型** (唯一权威) | `OriginalClientShellProps` :131 · `SceneAssetReadiness` :27 · `BevyEntityRenderState` :41 · `BevyMapRenderState` :94 |
| `apps/web/app/components/original-client-shell-flow.ts` | 屏幕→音乐、标签 helper | `desiredMusicForScreen` · `LOGIN_TRANSITION_FRAME_MS` · `ORIGINAL_AUDIO` |
| `apps/web/app/page.tsx` | **挂载点 + 传 props** | `<OriginalClientShell ...>` :11107 |
| `apps/web/app/page.tsx` | 并列的工具窗面 (对照) | `<ExtraWindows ...>` :11240 |
| `apps/web/app/components/original-client-overlays.tsx` | login / select 屏 + HUD | `LoginOverlay` · `SelectOverlay` · `MainHud` |
| `apps/web/app/components/original-client-game-ui-scene.tsx` | game 屏 HUD + **背包/角色窗在这里挂** | `GameUiScene` · `<InventoryWindow>` :364 · `<CharacterWindow>` :389 |

> **背包/角色面板不在 shell 里直接画**：shell 把 `showInventory/showCharacter/activeInventoryTab/
> activeCharacterTab` 等传给 `<GameUiScene>`（:2232），GameUiScene 再条件渲染 `InventoryWindow`
> (game-ui-scene.tsx:364) 和 `CharacterWindow` (:389)。所以「背包属于 shell 这块」=「经由 shell→GameUiScene」。

## 数据流 (How it threads the layers)

Shell 处在 5 层数据流的**末端渲染侧**和**动作发起侧**，本身不碰协议层：

**Inbound（state 流入，shell 只读 props）**
```
ServerPacket → gateway server_packet_to_event (web.rs) → page.tsx case "X": → updateWorld (page.tsx:1436)
  → world.* / world.stage5Systems.* (worldRef.current)
  → page.tsx render: <OriginalClientShell world={world} player={self} ... /> (page.tsx:11107)
  → shell JSX (:2019) 选屏 + 算 viewportEntitySprites / viewportMapSprites → GameSceneBackdrop / GameUiScene
```
Shell 拿到的是 page.tsx 已经派生好的视口数据：`viewportEntities` / `viewportTiles` / `selectedEntity`
/ `player`（= `self`）都是 props，不是 shell 自己从 packet 算的。

**Outbound（玩家动作，全部 via `on*` prop）**
```
shell 内某个 UI 事件 (click tile / 按键 / 子窗 onUseItem)
  → on*Callback prop  (e.g. onViewportTileClick / onUseItem / onSendChat)
  → page.tsx 里对应实参 (page.tsx:11160-11238)，多数直接 send({type,...}) 或调一个 handler
  → send(command) (page.tsx:4026) → gateway browser_command_to_action (web.rs:2570)
  → ClientPacket / SessionAction → simulation
```
例：`onSendChat={(message) => send({ type: "chat", message })}` (page.tsx:11171)；
`onLogout={() => send({ type: "logOut" })}` (page.tsx:11174)；`onSelectNpcDialogTarget` / `onSubmitNpcInput`
同理 (page.tsx:11233-11234)。背包动作 (`onUseItem` 等) 指向 page.tsx 的 `useItem`/`dropItem`/… handler
(page.tsx:11178-11193)，它们内部才 `send`。

Shell 内部唯一“自己产生”的输入是**键盘/指针视口控制**（移动、目标动作、belt 快捷键），但它们也只
是调 `on*` prop，不直接 send：
- 移动键（WASD/方向键）→ `dispatchKeyboardMoveInput` (:725) → `onViewportDirectionIntent` / `onViewportDirectionStop`
- 选中目标时 Space/Enter → `onPrimaryTargetAction`；`F` → `onApproachTarget` (handleShortcutKey :675)
- 数字键 1–6 → 读 `world.beltItems` 找 slot → `onUseItem` (:707-718)
- 鼠标点视口格子 → held-pointer 逻辑 → `onViewportTileClick` / `onViewportTileSecondaryAction`（右键）

## 状态形状 (State shape)

**Props 读的（page.tsx 拥有）** — 全部定义在 `original-client-shell-types.ts:131` `OriginalClientShellProps`：
- 屏幕/连接：`screen: ClientScreen`("login"|"select"|"game")、`runtimePhase/runtimeMessage: string`、
  `wsState: string`、`reconnectStatus: GatewayReconnectStatus`。
- 世界/玩家：`world: DisplayWorld`、`player: DisplayEntity|null`、`selectedEntity`、`sortedEntities`、
  `viewportEntities: Array<DisplayEntity & {dx;dy}>`、`viewportTiles`、`targetDistance: number|null`。
- 预测移动：`predictedPlayerPosition`、`getLivePlayerRenderPosition?(): PredictedPlayerMotion|null`（关键，见坑）。
- 渲染器握手：`sceneInteractionReady`、`bevyEntityRendererReady`、`bevyRuntimeBackend: "webgpu"|"webgl2"|null`。
- 登录/选角：`accountId`、`password`、`characters: SelectCharacterEntry[]`、`selectedCharacterIndex`、
  `suiWallets`、`walletPickerOpen`、`loginBusy`、`loginError`。
- 窗口开关（page.tsx 拥有的 React state，shell 只转发给 GameUiScene）：`showInventory`、`showCharacter`、
  `activeInventoryTab: InventoryTabKey`、`activeCharacterTab: CharacterTabKey`、`storageServiceOpenVersion: number`。
- 回调上行（state-changing）：`onBevyEntityRenderStateChange` / `onBevyMapRenderStateChange` /
  `onSceneAssetReadinessChange`——shell 把它算好的 Bevy 渲染状态/资源就绪度推回 page.tsx。

**Shell 自己的本地 React state / refs（纯渲染态，不进 world）**：
- `motionNow` (:465) — 渲染时钟，~30Hz rAF + 100ms fallback timer (:830)；驱动插值/重连倒计时/聊天气泡过期。
- `loginTransitionFrame` (:463)、`sceneSpriteFrameIndex` (:464, 120ms 动画 tick)、`stageScale` (:469, 适配 1024×768)。
- `sceneSpriteLibraries` (:466) + `missingSceneSpriteLibrariesRef`/`sceneSpriteLibraryInFlightRef` — 精灵库懒加载缓存。
- `bevyEntityAtlas` / `mapAtlasIndex` / `mapGpuFailed` — GPU 图集状态。
- `heldKeyboardMoveKeysRef` (:500)、`heldScenePointerRef` (:499)、`latestMoveInputRef` (:502) — 输入态。
- `entityMotionSnapshotsRef` (:493)、`chatBubbleStateRef` (:497)、`renderPlayer` (派生 :888)。

## 坑 & 不变量 (Invariants & gotchas)

- **presentation-only 铁律**：shell 内**没有** `socketRef` / `new WebSocket` / `send` / `updateWorld` /
  `worldRef`（已 grep 确认）。要发包，**只能**经 `on*` prop。在 shell 里直接 `send` 会绕开 page.tsx 的
  乐观更新/序列化，是评审必拦项。子窗（Inventory/Character）同理——它们也只有 `on*` prop。
- **`player` prop 即 `self`**：page.tsx 传 `player={self}` (page.tsx:11115)，是服务器权威自身实体。
  渲染用的是**派生的 `renderPlayer`**（shell :888），它把权威位置替换成 `getLivePlayerRenderPosition()`
  的预测位置——**但仅当 lead ≤ `MAX_PREDICTED_PLAYER_LEAD_TILES`(=2, app/components/original-client-scene-layout.ts:21)**，否则退回 `player`。
  这个 clamp 是移动「overshoot/snap」history 的一道闸：预测领先服务器超过 2 格就不再用预测位置渲染，
  避免渲染穿越未走的服务器格再被拉回。改动渲染位置逻辑前先读 movement-overshoot 记忆。
- **`predictedPlayerPosition` 传的是 `null`** (page.tsx:11116)；真正的预测走 `getLivePlayerRenderPosition`
  回调（每帧从 `worldRef.current` 现算 + `preserveCrystalSelfRenderPosition`），并在 page.tsx 侧也按
  `MOVEMENT_LOCAL_RENDER_MAX_LEAD_TILES` clamp 一次 (page.tsx:11137-11138)。两层都 clamp。
- **Bevy canvas 必须先于 runtime 存在**：shell mount 后 dispatch `mir2:bevy-canvas-ready` + 置
  `window.__mir2BevyCanvasReady=true` (:543-551)。runtime 在此之前 boot 会 panic `bevy_winit`
  ("Cannot find element")。这是 shell 必须 `ssr:false` 懒挂的原因。
- **隐藏 tab 暂停 rAF** → `motionNow` 不再前进 → 场景不刷新（QA 会看到 "Loading map…"/黑地板）。100ms
  fallback timer (:841) 只救重连倒计时/气泡过期，不救插值。验收时保持前台。
- **`sceneInteractionReady` 控 "Loading map…" 遮罩** (:2295)：page.tsx 传
  `screen !== "game" || initialSceneAssetsReady` (page.tsx:11117)。它由 preload 成功**或** 5s partial-ready
  超时翻 true，所以遮罩不会永久卡——除非精灵库加载被错误丢弃（曾有 bug：旧 `Promise.all + disposed` guard
  在 world.entities 每次变就丢在途加载，导致遮罩永挂；现改为独立加载 + in-flight set，见 :985-1010 注释）。
- **三套渲染路径（Bevy / DOM-GPU / DOM-img）择一画地图+实体**，由一组 `?flag`/localStorage 开关决定
  （`bevyMapActive` :1337、`mapGpuActive` :1339、`hideDomEntitySpritesForBevy` :1315）。默认 Bevy。
  "地图永不画两遍/永不空白" 是不变量：任一 GPU 路径 fail（`mapGpuFailed`/`webGl2EntityAtlasFailed`）就回落 DOM。
- **`world.*` 写入永远走 `updateWorld`(page.tsx:1436)，不在 shell**。shell 拿到的 `world` 是只读 props 快照。

## 如何扩展 (How to extend / add to this area)

**给 shell 加一个新的玩家动作（新按钮/新交互）** — 遵守 additive/optional + presentation-only：
1. `original-client-shell-types.ts` — 在 `OriginalClientShellProps`(:131) 加 `onFoo?: (...) => void`
   （**optional**，别破坏现有 `DisplayWorld`/调用方）。
2. `original-client-shell.tsx` — 在解构 props 处 (:337) 加 `onFoo`，在 JSX/事件 handler 里**调用** `onFoo(...)`
   （绝不在此 `send`）。若要传给子窗，作为 prop 透传给 `<GameUiScene>`(:2232) 或相应 overlay。
3. `apps/web/app/page.tsx` — 在 `<OriginalClientShell>`(:11107) 实参里接 `onFoo={...}`，指向一个
   handler 或内联 `() => send({ type:"fooCmd", ... })` (send at page.tsx:4026)。
4. 若是新协议命令，按 apps/web/CLAUDE.md 的 outbound 配方继续到 gateway `web.rs:2570` + protocol + sim。

**给 shell 加一个新的“屏幕”或视口覆盖层（纯渲染，无新包）**：
1. `original-client-shell.tsx` — 在主 `return`(:2019) 的 `.client-stage-frame` 内、参照
   `screen === "game" ? (...)` 模式加一个 `screen === "..." ? <NewOverlay .../> : null`。屏幕枚举在
   `lib/original-ui.ts` 的 `ClientScreen`。
2. 新覆盖层组件放 `app/components/original-client-*.tsx`，**只收 props**，业务态留在 page.tsx。
3. 若覆盖层需要新的 world 字段，按 inbound 配方先在 protocol/sim/gateway/page.tsx 落地（optional field），
   再作为 prop 传进来。

**给背包/角色窗加新动作**：改的是 `GameUiScene` → `InventoryWindow`/`CharacterWindow` 的 prop 链
（game-ui-scene.tsx:364/389），但**入口仍是 shell props**——先在 `OriginalClientShellProps` 加 `on*`，
shell 透传给 `<GameUiScene>`(:2232)，GameUiScene 再透传给子窗。详见 inventory.md。

## 相关 (Related)

- [`page-tsx-map.md`](./page-tsx-map.md) — page.tsx 块图；`send`(:4026)/`updateWorld`(:1436)/主 JSX(:11104) 锚点。
- [`protocol-cross-layer.md`](./protocol-cross-layer.md) — 5 层 wiring + 加功能完整配方（出入站两向）。
- [`world-scene-render.md`](./world-scene-render.md) — 视口里**怎么把地图+实体画到屏幕**（scene blueprint / atlas / Bevy 交接），shell 调用的 `buildViewportEntitySprite`/`buildViewportMapSprites` 等在那。
- [`inventory.md`](./inventory.md) — 背包/腰带/装备 state + item-action 命令（shell→GameUiScene→InventoryWindow 链的下游）。
- [`stage5-social.md`](./stage5-social.md) — `<ExtraWindows>`(page.tsx:11240) 那一面的社交/工具窗（与 shell 并列，不在 shell 里）。
- [`combat-feedback.md`](./combat-feedback.md) — 飘血字 + hit-flash 覆盖层（`OriginalClientSceneOverlays`，shell:2175 挂）。
- [`audio-vfx.md`](./audio-vfx.md) — shell 的屏幕→BGM (`desiredMusicForScreen`) + 登录音效 + 法术特效图集。
- 源码：`apps/web/app/original-client-shell.tsx` · `app/components/original-client-shell-types.ts` ·
  `app/components/original-client-game-ui-scene.tsx` · `app/components/original-client-overlays.tsx`。
