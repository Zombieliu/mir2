# 世界 / 场景渲染 + 资源管线 + Bevy 接线 — client map

> 客户端「前置铺垫」文档之一。索引与「如何加功能」配方见 apps/web/CLAUDE.md。

## 这块是干什么的 (What it does)

把一张 Crystal 地图 + 上面的实体「画到屏幕上」的整条链路：从 server 端把 `.map` 二进制解析成
`OriginalMapRegion`（每格 back/middle/front/tileAnimation 引用一个 PNG 帧），经 `/api/scene/crystal`
缓存路由送给 client，存进 `world.originalMapRegion`，再由 producer 组件折成 GPU draw-list / atlas 页，
通过 wasm setter 推进 Bevy WASM runtime（实际的画布渲染器）。

The render pipeline is **split server/client**: the heavy `.map` parse + per-frame PNG export runs in
Node (server-only `crystal-map-loader.ts`); the client only ever fetches a JSON `SceneBlueprint`,
holds it in world state, and feeds two derived streams (map tiles + entities) to the Bevy renderer.
Bevy is the canvas; React/`page.tsx` is the orchestrator + DOM-fallback renderer.

## 入口在哪 (Entry points)

| 文件 File | 作用 Role | 关键符号 Key symbols (file:line) |
|---|---|---|
| `apps/web/lib/crystal-map-loader.ts` | **server-only**。解析 `.map`（type 0–7/100），逐帧导出 `/original-map/<lib>/<n>.png`，产出 `OriginalMapRegion` | `loadCrystalSceneBlueprint` (338), `parseMapBytes` (404), `exportMapRegion` (769), `exportFrame` (933), `parseLibrary` (1225), `CrystalResourceMissingError` (193) |
| `apps/web/lib/scene-blueprint-cache.ts` | 蓝图缓存（mem 128 + 磁盘 `.next/cache`）按 16×17 chunk 取整 key | `loadCachedCrystalSceneBlueprint` (39), `createSceneCacheKey` (114), `normalizeSceneBlueprintRequest` (134) |
| `apps/web/app/api/scene/crystal/route.ts` | HTTP 入口；200=蓝图，**424**=`resource_missing` | `GET` (15) |
| `apps/web/lib/scene-types.ts` | 蓝图/region/cell 的线型 | `SceneBlueprint` (99), `OriginalMapRegion` (69), `OriginalMapCell` (51), `OriginalMapSprite` (45) |
| `apps/web/lib/world-model/` | framework-agnostic world store + 类型 + snapshot emitter | `WorldState` (types.ts:357), `createWorldStore` (store.ts:193), `createSnapshotEmitter` (snapshot-emitter.ts:61) |
| `apps/web/app/page.tsx` | 取景 effect、`MapChanged` 处理、Bevy boot + 推送 | `toBevyWorldSnapshot` (1306), 场景 effect (3586), `bootRuntime` (3405), `handleBevyMapRenderStateChange` (1889), `RuntimeModule` (153), `shouldReloadCrystalScene` |
| `apps/web/lib/map-atlas-manifest.ts` | GPU map atlas 索引（`(lib#frame)→page` O(1)） | `loadMapAtlasIndex` (65), `buildMapAtlasIndex` (41), `mapAtlasRectKeyForPath` (85) |
| `apps/web/app/components/original-client-scene-map-rendering.tsx` | 把 region cells 折成 `MapTileDraw[]`（GPU）+ `uncovered`（DOM 兜底） | `buildMapTileDrawList` (201) |
| `apps/web/app/components/original-client-shell-types.ts` | producer→runtime 的状态形状 | `BevyMapRenderState` (94), `BevyEntityRenderState` (41) |
| `apps/web/lib/asset-cache-packs.ts` | 登录/spawn 预热包（含 scene prewarm URL） | `ASSET_CACHE_PACKS` (19) |
| `apps/game-client/runtime/src/lib.rs` | **Bevy WASM 渲染器**；wasm setter + ECS 系统 | `boot_mir2_runtime` (599), `set_mir2_world_state` (482), `set_mir2_map_render_state` (535), `WorldSnapshot` (179), `MapRenderState` (388), `MapTile` (425) |
| `apps/game-client/runtime/src/{motion,interpolation}.rs` | 实体平滑（lerp / sub-cell glide） | `motion::EntityMotionTable`, `interpolation::SnapshotBuffer` |

## 数据流 (How it threads the 5 layers)

### A) 切图 / 进图（inbound，触发整条渲染链）

1. **simulation** → `ServerPacket::MapChanged`（或首次 `MapInformation`）。
2. **gateway** `server_packet_to_event` (web.rs:3610)：`MapInformation` 有手写 arm（web.rs:3815，发
   `mapIndex/fileName/title/miniMapIndex/bigMapIndex/music/spawnFlags`，**无** location/direction）。
   `MapChanged` 没有手写 arm——它走末尾 `other =>` 泛型分支 `typed_packet_event_detail`
   （`serde_json::to_value`，ServerPacket 上 `#[serde(rename_all_fields = "camelCase")]`，packets.rs:1916）
   → JSON camelCase：`{ packet:"MapChanged", payload:{ fileName, title, miniMap, bigMap, location, direction, …, typed:true } }`。
   （`MapChanged` 协议结构见 packets.rs:2021。）
3. **page.tsx** `case "MapChanged"` (8156)：算 `mapChanged = normalizeMapFileName(fileName)!==当前`，
   写 `mapFileName`/`mapTitle`/`miniMapIndex`，且 `mapChanged` 时**清空** `entities`(保留 self)/`groundDrops`/
   `projectiles`/`damageFloaters`/`sceneView`/`terrainPatches`/`originalMapRegion`(→null)。这是渲染重载的扳机。
4. **取景 effect** (page.tsx:3586，依赖 `[self?.x, self?.y, world.mapFileName]`)：算 `sceneKey = map:chunkX:chunkY`，
   `shouldReloadCrystalScene` 判定（`originalMapRegion===null` 或越出 `playBounds` margin 即重载）→
   `fetch('/api/scene/crystal?map=&x=&y=&width=&height=')`。
5. **route** → `loadCachedCrystalSceneBlueprint` → `loadCrystalSceneBlueprint`（解析 `.map` + `exportMapRegion`）。
   命中缓存返回头 `X-Mir2-Scene-Cache: hit|miss|bypass`。
6. **回填** `applySceneBlueprint`（3602）`updateWorld` 把 `sceneView/terrainPatches/originalMapRegion` 合进 world。
   424 时若 `mapFileName==="0"` 回落 `/api/scene/starter`。

> 注意：`OriginalMapRegion` 不是 packet——它是 client 自己向 `/api/scene` 拉的派生资源，**不**走 stage5 adapter。

### B) world → Bevy（outbound 渲染推送，两路）

- **动态 world**：`worldStoreRef`(world-model store) 与 React `world` 同步 → `createSnapshotEmitter`
  (page.tsx:3722，`runtimePhase==="running"` 才起) 每 `WORLD_SNAPSHOT_INTERVAL_MS=33ms` 用
  `select: toBevyWorldSnapshot`（只取 mapTitle/playerObjectId/selectedObjectId/sceneView/terrainPatches/
  decorObjects/**entities**/mineNodes——丢掉巨大的 `originalMapRegion`+背包/UI）→ dedupe by JSON →
  `runtime.setMir2WorldState(json)` → runtime `WorldSnapshot` (lib.rs:179) → ECS `ingest_pending_world_state`。
- **地图瓦片 / atlas**：producer 组件（shell 下，`buildMapTileDrawList` original-client-scene-map-rendering.tsx:201）消费 `world.originalMapRegion`
  + `loadMapAtlasIndex()`，发 `onBevyMapRenderStateChange(BevyMapRenderState)` → page.tsx
  `handleBevyMapRenderStateChange` (1889)：每个 atlas 页 RGBA 只 `setMir2MapRenderAtlas` 上传一次
  （`uploadedBevyMapAtlasKeysRef` 去重），其余几何 JSON 走 `setMir2MapRenderState` →
  runtime `MapRenderState`/`MapTile` (lib.rs:388/425) → `sync_map_render` 增量 diff 重建瓦片实体。
  实体同理走 `setMir2EntityRenderState` / `setMir2EntityRenderAtlas`。
- **没有 outbound BrowserCommand 属于本区**：移动/点击在 movement-controller，不在渲染管线内。

## 状态形状 (State shape)

`world.*`（`WorldState`，types.ts:357 / page.tsx 内联同构）本区相关键：

```ts
mapTitle: string | null
mapFileName: string | null            // normalize 后是 "0".."N"（去 .map / 路径）
miniMapIndex / bigMapIndex: number | null
sceneView: { center:{x,y}, width, height } | null
terrainPatches: TerrainPatch[]        // 程序化色块兜底（非 Crystal 真图）
decorObjects: DecorObject[]           // 同上，程序化装饰
originalMapRegion: OriginalMapRegion | null   // ← 真·Crystal 地图：sprites{} + cells[] + bounds
entities: WorldEntity[]               // 推给 Bevy 的实体（含 movement/attack/struck/die 动画窗口）
mineNodes: {x,y,stage}[]
```

`OriginalMapRegion`（scene-types.ts:69）：`sprites: Record<id, {kind,drawMode,frames:[{path,w,h,offX,offY}]}>`，
`cells: [{x,y,back?,middle?,front?,tileAnimation?,blocked?}]`（值=sprite id），`regionBounds`/`playBounds`，
可选 `missingAssets[]`（graceful 模式跳过的资源诊断）。`cellWidth=48 cellHeight=32`。

Bevy 侧镜像（lib.rs）：`WorldSnapshot`(179) / `MapRenderState`(388) / `MapTile`(425，`rectKey` 经 `#[serde(rename)]`)。
producer→runtime 的中间态：`BevyMapRenderState` / `BevyEntityRenderState`（shell-types.ts:94/41）。

本区**不写** `world.stage5Systems.*`（那是社交/交易/拍卖窗口的域）。

## 坑 & 不变量 (Invariants & gotchas)

- **资源在 git 里但 Vercel 不发**：`apps/web/.vercelignore` 第 10–19 行 strip 掉 `public/original-map/**/*.png`
  + 多个 `original-ui/**`（`prune-vercel-output-assets.mjs`），生产只从 **R2** (`mir2.obelisk.build`) 发。
  在线 `net::ERR_FAILED` 但 git 里有 → **R2 release 旧/不全**，不是代码 bug。
- **`crystal-map-loader.ts` 是 `import "server-only"`**：用 `node:fs`/`node:zlib`，**只能**在 route / RSC 跑，
  client 永远 import 不到。client 唯一入口是 `fetch('/api/scene/crystal')`。
- **424 = `resource_missing`，不是 500**：strict 模式（`MIR2_STRICT_ASSET_RESOLUTION=1`，CI/release gate）下任一缺帧
  即整景 424；运行时默认 **graceful**——跳过缺帧、记 `originalMapRegion.missingAssets`、头 `X-Mir2-Missing-Asset-Count`。
  page.tsx 对 424 只在 `map==="0"` 时回落 starter，否则抛错记日志。
- **推给 Bevy 的快照是投影过的**：`toBevyWorldSnapshot`(1306) **故意丢掉** `originalMapRegion`/背包/UI——
  整 world 每 33ms stringify 曾把主线程打满（~54ms/帧）。要让 Bevy 看到新字段，必须同时加进这个投影。
- **emitter dedupe 不带 `clientTimeMs`**：否则每 tick 都「变了」。`clientTimeMs` 只在真推送时盖章
  （snapshot-emitter.ts:78–82），且 runtime 当前用 **本地收包时刻**当插值时钟（lib.rs:196–202），不信浏览器钟。
- **GPU atlas 覆盖不全 → DOM 兜底**：`buildMapTileDrawList` 把 atlas 里**没有**的帧丢进 `uncovered`，
  由 DOM `<img>`（webgl2-map-atlas-layer）渲染。atlas manifest 缺失/不可解析时 `loadMapAtlasIndex` 返回 null，
  整图退化为 DOM 路径（不黑屏，但慢）。已知 ~156k 真瓦片不在 27-lib atlas 内（见 sibling perf 笔记）。
- **atlas RGBA 每页只上传一次**：`uploadedBevyMapAtlasKeysRef`；切 runtime backend / reuse 时这些 ref 会
  `.clear()`（page.tsx 3430/3514）以便重传，漏 clear 会导致换图后 atlas 不刷新。
- **`mapChanged` 清场要保留 self entity**：`store.setMap` / `case "MapChanged"` 都只留 `playerObjectId` 那一个，
  否则切图瞬间自己消失、相机失去 follow 目标。
- **缓存 key 按 chunk 取整**：`SCENE_CACHE_CHUNK_WIDTH/HEIGHT = 16/17`，center 会被吸到 chunk 中心
  （scene-blueprint-cache.ts:144），所以同一 chunk 内移动**不**重新解析。改这俩常量或 `SCENE_CACHE_SCHEMA_VERSION`
  会整体 cache-bust。
- **Type-1/4 地图带 XOR 解码**：`backImage ^= 0xAA38AA38`、宽高/索引 `^= xor`（loader 464/555）。改解析器先核对
  Crystal `MapControl.cs` 的同名常量再动。
- **Bevy ECS 顺序固定 chained**：`boot_mir2_runtime` (599) 的 `Update` 链 ingest→motion→sync 有依赖，乱序会丢帧。

## 如何扩展 (How to extend / add to this area)

**给 Bevy 渲染加一个新世界字段（如新的地面标记）——典型增量改动：**

1. **协议/sim**：若来自服务器，先在 `packages/protocol` 加 `ServerPacket` 字段，sim 发包；否则跳过（client 派生）。
2. **gateway** `apps/gateway/src/web.rs` `server_packet_to_event`：把它 JSON 化为 camelCase payload。
3. **world 类型**：`apps/web/lib/world-model/types.ts` 的 `WorldState`（+ page.tsx 内联同构副本）加**可选**字段，
   `DEFAULT_WORLD_STATE` 给默认值；需要命名 mutation 时在 `store.ts` 加（参考 `upsertGroundDrop`）。
4. **page.tsx case**：在对应 `case "X"` 里 `updateWorld` 写入；切图清场处（8156 与 `store.setMap`）决定它是否随图清空。
5. **投影**：把字段加进 `toBevyWorldSnapshot`(1306)——**否则 Bevy 永远收不到**（最常踩的坑）。
6. **runtime**：`lib.rs` 的 `WorldSnapshot`(179) 加 `#[serde(default)]` 字段，写一个 `sync_*` 系统并挂进
   `boot_mir2_runtime` 的 `Update` chain（参考 `sync_mine_nodes`）。

**给地图加一个新的精灵层 / 图块来源：**

1. `scene-types.ts`：`OriginalMapSprite.kind` / `OriginalMapCell` 加层（可选键）。
2. `crystal-map-loader.ts`：在 `exportMapRegion` 的 cell 循环里 `registerSprite(...,"newKind")`，
   并写 `newKindLayerForCell`（参考 `frontLayerForCell`）；PNG 路径走 `exportFrame` 的 `/original-map/<lib>/<n>.png` 约定。
3. 若要进 GPU 快路径：确保该帧被 `build-map-atlas-pack.mjs` 打进 atlas（否则自动走 DOM `uncovered`，仍能显示）。
4. producer（`buildMapTileDrawList` 等）一般自动吃新 `sprites`/`cells`，无需改 Bevy。
5. 类型检查：`npx tsc --noEmit` 必须 0；新引用的 PNG 若本地没有，记得它生产时来自 R2（见 gotchas）。

> 增量原则：新字段一律 **optional + 向后兼容**，绝不破坏 `DisplayWorld` / 既有消费者；推 Bevy 前先过 tsc。

## 相关 (Related)

- `docs/ARCHITECTURE-CURRENT.md` — 系统总览
- 同目录 sibling docs（若存在）：world-state / packet-handling / asset-pipeline 细分文档
- 关键源码：`apps/web/lib/crystal-map-loader.ts`、`apps/web/lib/world-model/`、
  `apps/web/app/components/original-client-scene-map-rendering.tsx`、`apps/game-client/runtime/src/lib.rs`
- 资产发布：`docs/ASSET-RELEASE-RUNBOOK.md`（R2 publish，决定在线能否看到图）
- 打包脚本：`apps/web/scripts/build-map-atlas-pack.mjs`（GPU map atlas）、`prune-vercel-output-assets.mjs`（strip）
