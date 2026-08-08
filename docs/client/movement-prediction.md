# 移动预测 / 和解引擎 — client map

> 客户端「前置铺垫」文档之一。索引与「如何加功能」配方见 apps/web/CLAUDE.md。

## 这块是干什么的 (What it does)

客户端在本地**预测**自身角色移动（点亮 sprite + 移动镜头），同时把 `walk`/`run` + 方向发给服务器，并在服务器权威位置回包时**和解** (reconcile) 预测与真相。它是整个客户端 bug 最密集的区域——已知两类历史 bug：**overshoot-then-snap（预测越界后回弹）** 和 **held-key stall（按住方向键卡 1–3 秒后跳 2 格）**。两者各有一条防御不变量，见 [坑 & 不变量](#坑--不变量-invariants--gotchas)。

Self-movement is locally predicted; the predicted tile renders the player sprite + drives the camera, leading the authoritative server tile by at most **2 tiles**. The server echoes the real position via `UserLocation`/`ObjectWalk`/... and the engine reconciles. **No `moveTo` packet exists** — the client only ever sends `walk`/`run`/`turn` + a `direction`; the server resolves the tile. Crystal authority: `Client/MirScenes/GameScene.cs` (`CanMove`/`CanRun`/`MoveTime`/`NextRunTime`, lines 41–42, 1190, 3385).

## 入口在哪 (Entry points)

| 文件 File | 作用 Role | 关键符号 Key symbols (file:line) |
|---|---|---|
| `apps/web/app/components/original-client-movement-controller.ts` | **纯函数核心** (testable): 移动模式、pending-move 构造、ack 和解、**lead 钳制**、cadence 常量 | `CRYSTAL_MOVE_DELAY_MS=600` :38 · `clampMovementLeadToCap` :108 · `reconcileMovementAck` :138 · `reconcileMovementSnapshot` :182 · `canSendMovement` :208 · `createPendingSelfMove` :78 |
| `apps/web/app/page.tsx` | 出站发包 + 预测推进 + 入站和解 + 渲染位置裁决 (engine) | `queueCrystalMoveIntent` :4790 · `trySendQueuedCrystalMove` :4823 · `sendCrystalTurn` :4799 · `reconcileMovementPlanWithServer` :10026 · `applyCrystalInputCorrection` :10356 · `crystalMovementActionForDirection` :10787 · `preserveCrystalSelfRenderPosition` :3310 · `chooseCrystalSelfRenderPosition` :12412 · `setPredictedPlayerMotion` :1945 |
| `apps/web/app/page.tsx` (rAF) | 每帧泵 — 把排队 intent 在 600ms cadence 闸门下变成发包 | `tickMovementPlan` :3915 (定义于 `useEffect`, deps `[screen, wsState]`) |
| `apps/web/app/page.tsx` (入站) | 服务器自身位置回包 → 合并 worldRef + ack 和解 | `case "UserLocation"`…`"ObjectSitDown"` :6674 → `reconcileSelfMovementAck` :2527 |
| `apps/web/app/page.tsx` (视口交互) | 点击/方向意图处理器 | `handleViewportDirectionIntent` :6156 · `handleViewportDirectionStop` :6174 · `handleViewportTileStepAction` :6090 · `moveToTile` :4948 |
| `apps/web/app/page.tsx` (refs) | 所有移动可变状态 | `movementPlanRef` :1359 · `pendingSelfMoveRef` :1372 · `queuedMoveIntentRef` :1374 · `predictedPlayerPositionRef` :1377 · `movementBlockedStepsRef` :1360 · `directionStep*Ref` :1361-1368 · `crystalRunPrimedUntilRef` :1364 · `nextMoveSendAtRef` :1376 |
| `apps/web/app/page.tsx` (常量) | cadence / lead / 记忆 TTL | :887-918 — `MOVEMENT_LOCAL_RENDER_MAX_LEAD_TILES=2` :908 · `MOVEMENT_ROUTE_BLOCK_MEMORY_MS=5600` :912 · `MOVEMENT_PENDING_ACTION_MAX_AGE_MS` :898 |
| `apps/web/app/original-client-shell.tsx` | **键盘**按键 → 方向意图（注意：在 `app/` 下，不在 `components/`） | `dispatchKeyboardMoveInput` :725 · 100ms `setInterval(…, CRYSTAL_MOVE_INPUT_INTERVAL_MS)` :810 · `onViewportDirectionIntent` 接 `handleViewportDirectionIntent` (page.tsx:11213) |
| `apps/web/app/components/original-client-mobile-input.ts` | **摇杆**矢量 → 八方向 + walk/run（带死区/角度迟滞） | `mir2MobileMoveIntentFromVector` :105 · `mir2MobileDirectionFromVector` :59 |

> page.tsx 行号会随编辑漂移（codegraph 索引偏低 ~50–70 行）。以上行号均对当前盘上文件核对；用 `grep -n` 重新定位，别信旧行号。

## 数据流 (How it threads the layers)

**出站 OUTBOUND（预测 → 发包）:**
```
键盘 shell dispatchKeyboardMoveInput("held"|"edge")  ─┐  (100ms interval, original-client-shell.tsx:810)
摇杆 mobile-input mir2MobileMoveIntentFromVector      ─┤
点击地块 handleViewportTileStepAction (page.tsx:6090) ─┘
   → onViewportDirectionIntent → handleViewportDirectionIntent (page.tsx:6156)   [kind:"direction"]
   → 点远处地块 moveToTile (page.tsx:4948)                                         [kind:"target"]
   → queueCrystalMoveIntent (page.tsx:4790)  ← 写 queuedMoveIntentRef，清掉旧 plan/direction-step
   → trySendQueuedCrystalMove (page.tsx:4823)
        · canSendMovement 闸门：!pending && now≥nextMoveSendAt && now≥inputBlockedUntil
        · direction: crystalMovementActionForDirection(serverSelf, dir, mode, [], world)   ← 关键：传 []，只看 LIVE 阻挡
        · target:    crystalMovementActionTowardWithRouteHints(…, recentMovementBlockedSteps(…))  ← A* 用记忆绕行
        · 构造 pendingSelfMoveRef + setPredictedPlayerMotion(to, visualUntil) + nextMoveSendAt = now+600
        · send({ type:"walk"|"run"|"turn", direction })   (page.tsx send :4026)
   → gateway browser_command_to_action (web.rs:2570) → ClientPacket::Walk/Run/Turn → simulation
```

**入站 INBOUND（服务器权威位置 → 和解）:**
```
simulation Vec<ServerPacket> (UserLocation / ObjectWalk / ObjectRun / Pushed / UserDashFail / …)
   → gateway server_packet_to_event (camelCase JSON; web.rs)
   → page.tsx switch case "UserLocation"…"ObjectSitDown" (page.tsx:6674)
        · movementPointFromPacketPayload → {x,y,direction}
        · selfMovementPacket? (UserLocation/Pushed/UserDash*/UserAttackMove) ⇒ objectId = playerObjectId
        · updateWorld(…) 把权威坐标合并进 worldRef.current.entities (page.tsx:6758, withCrystalSelfPacketMovement)
        · reconcileSelfMovementAck({x,y,direction}, packet, now) (page.tsx:2527)
              → reconcileMovementAck (controller.ts:138):
                    ack==pending.to ⇒ "confirmed"  → 清预测 + 续泵 trySendQueuedCrystalMove
                    UserDashFail / ack≠to ⇒ "correction" → applyCrystalInputCorrection 风格清空 + inputBlockedUntil=now+400
   → （click-to-target 还有一条平行的路线和解）reconcileMovementPlanWithServer (page.tsx:10026) 处理 A* plan 的逐步推进/重路由
```

`predictedSelf`（page.tsx:1783）= self 实体叠加预测坐标后的渲染实体；它既是镜头中心 `viewportCenter`（page.tsx:3352），也在 `displayEntities`（page.tsx:1802）里替换玩家 sprite。Bevy 每帧通过 `getLivePlayerRenderPosition`（page.tsx:11123）读 `predictedPlayerPositionRef`（ref 路径，不走 React state——见 `setPredictedPlayerMotion` :1959 的注释，**禁止 flushSync**）。

## 状态形状 (State shape)

全是 `useRef`（同步真相，packet handler 读 ref 不读 React state）。React state 只有派生 `predictedPlayerPosition`（喂选择/排序，非渲染关键路径）。

| Ref (page.tsx) | 类型 / 含义 |
|---|---|
| `queuedMoveIntentRef` :1374 | `QueuedMoveIntent \| null` — 待发意图 `{kind:"direction"\|"target", direction?, targetX?, targetY?, requestedMode, requestedAt, consumeAfterSend?}` (controller.ts:18) |
| `pendingSelfMoveRef` :1372 | `PendingSelfMove \| null` — 已发出、等 ack 的一步 `{from, to, direction, mode, sentAt, visualUntil}` (controller.ts:9) |
| `predictedPlayerPositionRef` :1377 | `PredictedPlayerMotion \| null` — 当前渲染的预测坐标（含 direction + 动画字段） |
| `predictedPlayerHoldUntilRef` :1378 | 预测保持到此时刻前不被 `setPredictedPlayerMotion(null)` 清掉（视觉补间窗口） |
| `nextMoveSendAtRef` :1376 | 下一次允许发包的最早时刻 = 上次发包 + `movementCommandDelayMs(mode)`（≈600ms cadence 闸门） |
| `crystalRunPrimedUntilRef` :1364 | run 只有在 `now ≤ runPrimedUntil` 时才生效（confirm 后 +`CRYSTAL_RUN_PRIME_MS`=1200；correction 清 0） |
| `movementInputBlockedUntilRef` :1363 | correction 后封锁发包到 `now+CRYSTAL_INPUT_CORRECTION_DELAY_MS`=400 |
| `movementBlockedStepsRef` :1360 | `MovementBlockedStep[]` — **仅 click-to-target A\* 的路线记忆**（`{fromX,fromY,direction,mode,at}`，TTL `MOVEMENT_ROUTE_BLOCK_MEMORY_MS`=5600） |
| `movementPlanRef` :1359 | `MovementPlan \| null` — click-to-target 的多步 A\* 计划（`actionX/Y`, `pendingX/Y`, `targetX/Y`, `nextStepAt`, `blockedSteps`） |
| `directionStep*Ref` :1361-1368 | **遗留** direction-step 队列状态；`tickMovementPlan` 每帧把它们清空（:3920-3925），现役路径不依赖它们 |
| `lastCrystalSelfRenderPositionRef` :1383 | 上一帧选定的渲染预测，`preserve*` 用作连续性基线 |

controller-state 视图：`readSelfMovementControllerState()`（page.tsx:2487）把 `{pending, prediction, nextMoveSendAt, runPrimedUntil, inputBlockedUntil}` 打包给纯函数；`applySelfMovementControllerState`（:2497）写回。

## 坑 & 不变量 (Invariants & gotchas)

- **不变量①（防 overshoot-then-snap）：渲染预测对服务器格的 lead 必须 ≤ `MOVEMENT_LOCAL_RENDER_MAX_LEAD_TILES`=2；越界时 *钳制到 cap*，绝不丢弃。** 早期 bug：`chooseCrystalSelfRenderPosition`（page.tsx:12412）把每个 over-cap 候选 *filter 掉* → 返回 null → 渲染器回退到服务器格 → 可见的「越界后回弹」。修复（#136）= 沿行进向量把候选钳到 cap（`clampMovementLeadToCap`, controller.ts:108），**两个裁决闸门都做**：`chooseCrystalSelfRenderPosition`（:12430-12434）和 `preserveCrystalSelfRenderPosition`（page.tsx:3334）。改这两处任何一个的钳制逻辑都会让 bug 复活。
- **残留风险（本分支未含 #148 的 per-frame easing）：** lead-clamp 只约束「预测领先服务器多远」，不约束「渲染格单帧移动多远」。当预测在未推进的服务器格两侧**反向**（如先 down-left 预测、玩家反向、再 up-right 预测），渲染格可能在一个掉帧里跨越服务器格跳 2 格（Chebyshev 2），lead-clamp 触不到。memory 记录 #148 用 `stepMovementTowardWithinCap`/`easeSelfRenderTile`（≤1 格/帧 easing）修了它——但**这两个符号在当前 worktree 不存在**（controller.ts 只有 `clampMovementLeadToCap`），说明本分支早于 #148。若要根治反向跨格，需移植 #148 的 per-frame 缓动。**服务器驱动的移动（sync/correction/teleport）不做缓动**——预测被丢弃后立即 snap，否则真传送会被慢走。
- **不变量②（防 held-key stall）：held/discrete DIRECTION 意图发包时只看 LIVE 阻挡（传 `[]`），绝不喂 `movementBlockedStepsRef` 记忆，也不 seed 它。** `trySendQueuedCrystalMove`（page.tsx:4872-4874）对 `kind:"direction"` 显式传 `[]`；source-blocked 时只有 `kind:"target"` 才 `rememberBlockedDirectionAtSource`（:4905-4906）。早期 bug（#144）：held 方向走了同一套 sticky 路线记忆（本为 click-to-target A* 绕行设计），一次瞬时碰撞 seed `{tile,dir}` 后 `movementStepBlockedByRecentCorrection` 在 5.6s TTL 内压制该格所有 held 发包，松手后变成 run 跳跃。改动 :4872 的 `[]` 或把 `rememberBlockedDirectionAtSource` 的 `kind==="target"` 守卫去掉都会复活 stall。注释就在 :4865-4871 和 :4901-4904，**别删**。
- **600ms cadence ≠ 100ms input interval。** shell 的 `setInterval`（original-client-shell.tsx:810）每 100ms 重试 `dispatchKeyboardMoveInput("held")`，但实际发包被 `canSendMovement`（controller.ts:208）的 `nextMoveSendAt`（=上次+`CRYSTAL_MOVE_DELAY_MS`=600）闸住，所以 held 实测约 ~1 包/600ms。Crystal 对应：input poll `MoveTime = CMain.Time + 100`（GameScene.cs:1190）、`CanMove`/`CanRun` 闸（GameScene.cs:42）、`NextRunTime = CMain.Time + 2500`（run-prime，GameScene.cs:3385，本端用 1200 的 `CRYSTAL_RUN_PRIME_MS`）。
- **没有 `moveTo` 包。** 出站只有 `walk`/`run`/`turn` + `direction`。点远处地块（`moveToTile`/`handleViewportTileStepAction`）在本地解析出下一格方向再发方向包；服务器决定落点。验证移动 QA 时别去 grep `moveTo` 协议。
- **`tickMovementPlan` 每帧清 plan + direction-step 队列**（page.tsx:3920-3925）再调 `trySendQueuedCrystalMove`——预测推进是靠 cadence 闸门里**重新发出下一步**，不是一个独立 predictor 累加器。所以「按帧 dt 归一化」对自身预测是红鲱鱼（per-frame 视觉偏移 `movementProgressRatio` 已在 `original-client-scene-motion.ts` 钳到 [0,1]）。
- **handler 读 `worldRef.current` 不读 React `world`。** 一个 microtask 内常有多包先于 React flush 到达；`currentAuthoritativeSelf()`（page.tsx:2509）默认读 `worldRef.current`。所有 world 写入走 `updateWorld`（page.tsx:1436），它同步写 `worldRef.current`（见 :6790）再 rAF-batch `setWorld`。
- **`setPredictedPlayerMotion` 禁止 flushSync**（page.tsx:1959 注释）：预测走 ref 路径每帧到渲染器，flushSync 会强制整个 ~12.7k 行 HomePage 同步重渲 ~200ms，饿死 Bevy。
- **隐藏标签页暂停 rAF** ⇒ `tickMovementPlan` 不跑、`setWorld` 不 flush（QA 会看到角色冻结 / 「Loading map…」）。验证移动时保持标签页前台。
- **QA verdict 易被污染。** `qa-load-stress.mjs` 的 `reproduced = snaps>0 || corr>0` 会被「服务器 move-validation 修正」「WS-resync 修正」误触（随机器负载放大）。判 overshoot 修复要看 **HITCH 阶段**的 snaps/maxLead，且在**凉机**单跑；判 held stall 要按下一格占用 + `movementInputBlockedUntil` + internal-vs-trailing 分类（墙/NPC 不释放 ≠ bug）。

## 如何扩展 (How to extend / add to this area)

遵循「新字段可选 + 向后兼容」；不要破坏 `PredictedPlayerMotion` / `MovementControllerState` 现有字段。

1. **改 cadence / lead / 记忆参数**：动 `original-client-movement-controller.ts` 的常量（`CRYSTAL_MOVE_DELAY_MS` :38 等）或 page.tsx 的 `MOVEMENT_*` 常量块（:887-918）。改 lead cap 必须同时影响 `chooseCrystalSelfRenderPosition`、`preserveCrystalSelfRenderPosition`、`predictedSelf`（:1791）、`getLivePlayerRenderPosition`（:11138）四处的 ≤cap 判断——它们都引用同一常量，改常量即可，别硬编码。
2. **加纯逻辑（可单测）**：先在 `original-client-movement-controller.ts` 写纯函数（如新的钳制/和解规则），导出后在 page.tsx import 调用。`npm run test:frontend-logic` 覆盖这些纯函数——加测试，别只手测。
3. **加一种新的出站移动包**：在 page.tsx 写 handler → `send({type:"<camelType>", direction})`（参 `sendCrystalTurn` :4799）；gateway `web.rs` `BrowserCommand`(:585) + `browser_command_to_action`(:2570) 加 arm → `ClientPacket`；`packages/protocol/src/packets.rs` 加 variant + `packet_id`；simulation `runtime/packets.rs` 加处理 arm 并回 `Vec<ServerPacket>`。
4. **加/改一个入站权威位置包的和解**：在 page.tsx switch（:6674 那一组 case）加 case 或扩展 payload 读取；确保它 (a) `updateWorld` 合并坐标进 `worldRef`，(b) 若是自身包则调 `reconcileSelfMovementAck`（:2527）。gateway `server_packet_to_event` 用 camelCase key 手写该 arm。
5. **改 held / 摇杆输入语义**：键盘在 `original-client-shell.tsx`（`dispatchKeyboardMoveInput` :725 + 100ms interval :810），摇杆在 `original-client-mobile-input.ts`（死区/迟滞纯函数，可单测）。两者最终都收敛到 `handleViewportDirectionIntent`（page.tsx:6156）→ `queueCrystalMoveIntent`——**不要**让 held 方向重新接触 `movementBlockedStepsRef`（见不变量②）。
6. 每次推送前：`npx tsc --noEmit`（0）+ `npm run test:frontend-logic` + `cargo fmt --all --check`。

## 相关 (Related)

- `docs/client/world-scene-render.md` — 预测坐标如何变成屏幕/镜头（`predictedSelf` → Bevy 快照）
- `docs/client/protocol-cross-layer.md` — 5 层 ServerPacket/ClientPacket 接线 + 加 feature 配方
- `docs/client/page-tsx-map.md` — ~12.7k 行 page.tsx 的块地图 + ServerPacket switch 分域
- `docs/client/combat-feedback.md` — 移动旁路的攻击/受击表现（同一 rAF / DOM 覆盖层）
- 源码：`apps/web/app/components/original-client-movement-controller.ts`（纯核心）· `apps/web/app/page.tsx`（引擎 :3310-3346, :4790-4957, :6674-6799, :10026-10174, :12412-12442）· `apps/web/app/original-client-shell.tsx`（键盘）· `apps/web/app/components/original-client-mobile-input.ts`（摇杆）
- 历史 bug repro 笔记（memory）：`movement-overshoot-snap-repro`（#136/#148）· `held-keyboard-movement-qa`（#144）
