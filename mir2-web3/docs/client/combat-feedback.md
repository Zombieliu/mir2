# 战斗反馈层（飘血 / 受击闪光 / 音效）— client map

> 客户端「前置铺垫」文档之一。索引与「如何加功能」配方见 apps/web/CLAUDE.md。

## 这块是干什么的 (What it does)

战斗「打击感」(combat juice) 全部活在 **DOM overlay 层**，不在 sprite 上。Bevy 是默认实体渲染器，它不画飘血字 / 受击高亮，所以这些反馈以绝对定位的 `<div>` 覆盖在 Bevy canvas 之上（renderer-independent）。三件事：(1) **飘血字** floating damage numbers，来自 `DamageIndicator` 包；(2) **受击闪光** hit-flash，一个 ~170ms 的白/红 brighten，由实体的 `struckStartedAt` 驱动；(3) **战斗音效**，大多由 **客户端自己**按 Crystal 公式从 `ObjectAttack`/`ObjectStruck`/`ObjectDied`/`ObjectMagic` 包触发（不是服务器推的）。

关键架构：所有反馈都不直接从 packet handler 调播放/渲染函数，而是 page.tsx 的 case handler 往 `gameBusRef`（一个进程内事件总线）`emit` 一个 `GameEvent`，由两个 subscriber（`vfx-subscriber` 画 DOM、`sound-subscriber` 出声）消费。HP 本身权威走 `ObjectHealth`/`HealthChanged`，飘血字**只是**一个短命的视觉浮标。

## 入口在哪 (Entry points)

| 文件 File | 作用 Role | 关键符号 Key symbols (file:line) |
|---|---|---|
| `app/components/original-client-scene-overlays.tsx` | **本区主渲染层**：飘血字 / 受击闪光 / 血条 / 选中环 / 目标读条，全是内联样式 div，跟着 sprite 走 | `OriginalClientSceneOverlays` :620, `DamageFloaters` :203, `HitFlashes` :261, `HIT_FLASH_MS` :44, `OVERLAY_STYLES` :411（`.scene-damage-floater` :571, `.scene-hit-flash` :609, `@keyframes scene-damage-rise` :603）|
| `lib/original-sound-triggers.ts` | Crystal 按动作算 sound id 的公式（怪物 roar/swing/die、玩家 swing/struck/性别死亡音） | `playEntityAttackSound` :63, `playEntityStruckSound` :82, `playEntityDieSound` :98, `playMagicSoundId` :117, `monsterImageFromBodyLibrary` :45 |
| `lib/vfx-fallback.ts` | 无图集时的程序化 CSS 法术特效：element×archetype 分类 + 每帧样式 + world→effect 采集器 | `classifySpellName` :401, `collectViewportFallbackVfx` :846, `fallbackVfxStyle` :494, `SPELL_TABLE` :221, `instantKindForArchetype` :591 |
| `app/components/original-client-scene-visual-layers.tsx` | 渲染 `FallbackVfxNode`（程序化特效 div）+ 拉图集（图集优先） | `FallbackVfxNode` :236, `useEffectAssets` :218, `loadEffectAssetsOnce` :201, fallback 采集调用 :369, 渲染 mount :560 |
| `lib/game-events/vfx-subscriber.ts` | `damageDealt`→飘血字、`entityStruck`→受击闪光（注入式 `VfxSink`，可测） | `registerVfxSubscriber` :52, `VfxSink` :37 |
| `lib/game-events/sound-subscriber.ts` | 把所有战斗/UI 事件接到 `original-audio` + `original-sound-triggers`（注入式 `AudioSink`） | `registerSoundSubscriber` :63, `makeRealAudioSink` :141 |
| `lib/game-events/events.ts` | 总线事件词表（discriminated union） | `DamageDealtEvent` :95, `EntityStruckEvent` :38, `EntityAttackEvent` :31, `EntityDiedEvent` :45, `CrystalDamageType` :24 |
| `app/page.tsx` | packet case + 浮标/闪光 state 写入 + 总线 emit + subscriber 注册 | `case "DamageIndicator"` :8077, `pushDamageFloater` :9192, `markEntityStruckFlash` :9257, `markWorldEntityStruck` :9270, `markWorldEntityDead` :9345, 总线注册 :1524 |

> 音频解析引擎本身（id→wav、prod allowlist、BGM 单例）+ 法术真·图集汇编在姊妹文档 `docs/client/audio-vfx.md`，本文不重复。

## 数据流 (How it threads the 5 layers)

**飘血字（inbound `DamageIndicator`）**——纯展示，HP 不在这条路上变：

1. **protocol**：`ServerPacket::DamageIndicator { damage: i32, damage_type: u8, object_id: u32 }`（`packets.rs:1979`，id=75）。`damage` 是 HP delta：负=受伤，正=回血/regen（Crystal 发的是 `armour - damage`）。
2. **gateway**：`DamageIndicator` 在 `web.rs` `server_packet_to_event` 里**没有专门 arm**，落到末尾的 `other =>`（`web.rs:5996`）经 `typed_packet_event_detail` 用 serde `rename_all = "camelCase"` 序列化 → JSON `{ type:"packet", packet:"DamageIndicator", payload:{ objectId, damage, damageType } }`。
3. **page.tsx**：`case "DamageIndicator"`（:8077）只做一件事——`gameBusRef.current!.emit({ type:"damageDealt", objectId, damage, damageType })`。**不**直接写 state。
4. **subscriber**：`registerVfxSubscriber` 的 `damageDealt` handler（`vfx-subscriber.ts:57`）调注入的 `sink.addDamageFloater`，page.tsx 注册时把它接到 `pushDamageFloaterFromBus`→`pushDamageFloater`（:9192），后者算出 `variant`(hit/miss/crit/heal) + `text` 并 push 进 `world.damageFloaters`（带 cap 48 + 过期裁剪）。
5. **component**：`OriginalClientSceneOverlays`（在 `app/original-client-shell.tsx:2175` 挂载，`damageFloaters={world.damageFloaters}`）→ `DamageFloaters`（:203）按 objectId 找到 sprite entry，定位到 sprite 顶部，靠 CSS `@keyframes scene-damage-rise` 上浮+淡出。无自有 timer：`motionNow` 时钟驱动，过期由 `expiresAt <= motionNow` 直接 return null，并在快照 merge（page.tsx:9993）里裁掉。

**受击闪光 + 战斗音效（inbound `ObjectStruck` / `ObjectAttack` / `ObjectDied`）**：

- `case "ObjectStruck"`（:6859）→ `markWorldEntityStruck`（:9270）`emit { type:"entityStruck", objectId }`；玩家自己被打是 `markPlayerStruck`（:9287）。
- `entityStruck` 同时被**两个** subscriber 消费：`vfx-subscriber`→`markEntityStruckFlash`（:9257）给实体打 `struckStartedAt = now`；`sound-subscriber`→`playEntityStruckSound`。`HitFlashes`（overlays :261）读 `entity.struckStartedAt`，年龄 < `HIT_FLASH_MS`(170) 时画一个 `mix-blend-mode:screen` 的高亮（玩家红、怪白）。
- `case "ObjectRangeAttack"`（:6863）/ 近战 attack → `emit entityAttack` → `sound-subscriber`→`playEntityAttackSound`；`markWorldEntityDead`（:9345）→ `emit entityDied`→`playEntityDieSound`。

> 出站 `BrowserCommand`→`ClientPacket`（玩家发起攻击/施法）不属于本反馈层——本层只消费 inbound 的「结果」包。

### 事件总线机制 (the bus, `lib/game-events/bus.ts`)

`createGameEventBus()`（:34）是一个**同步** typed pub/sub，零依赖、无 React/DOM：

- `emit(event)`：按 `event.type` 找到 handler set，**插入顺序**同步调用，然后再调所有 `onAny` handler。同步意味着 `emit` 返回时副作用（出声、写 state）已发生。
- `on(type, handler)` / `onAny(handler)`：返回**幂等** unsubscribe。`on` 用 `GameEventOf<T>` 收窄 handler 入参，call site 全推断。
- 一个事件可被**多个** subscriber 听（`entityStruck` 同时触发 vfx 闪光 + sound 撞击声）；`emit` 一次、扇出多处。
- 全部 `GameEvent` 变体见 `events.ts:112` 的 union；本反馈层用到的是 `damageDealt`/`entityStruck`/`entityAttack`/`entityDied`/`magicCast`。

## 状态形状 (State shape)

- `world.damageFloaters: DisplayDamageFloater[]`（page.tsx 内别名 `DamageFloater`，类型在 `original-client-types.ts:127`）：`{ key, objectId, text, variant: "hit"|"miss"|"crit"|"heal", isPlayerTarget, startedAt, expiresAt }`。MapChanged 时清空（page.tsx:8187）。`damageFloaterSeqRef`（:1337）保证 key 唯一。
- 受击闪光**不是独立集合**，是实体字段：`DisplayEntity.struckStartedAt?: number` + `struckUntil?: number`（由 `markEntityStruckFlash` 写）。同理 attack 动画用 `attackAnimation` + `attackStartedAt` + `attackUntil`。
- `gameBusRef.current: GameEventBus`（page.tsx:1332，懒初始化 `createGameEventBus()`）——所有反馈的中枢。subscriber 在 :1524 的 `useEffect` 里一次性注册（空 deps，靠 ref 闭包），返回 `unsubSound`/`unsubVfx` 清理。
- 程序化特效**无 React state**：`collectViewportFallbackVfx`（vfx-fallback.ts:846）每帧从 `viewportEntitySprites` + `viewportProjectiles`（tile-delta 空间）**派生**一次性 `FallbackVfx[]`，空闲帧返回 `[]`（零成本）。图集句柄 `useEffectAssets()`（visual-layers:218）模块级 memoize，永不重拉。
- 本层不读 `world.stage5Systems.*`（那是窗口/社交数据，与战斗反馈无关）。

## 坑 & 不变量 (Invariants & gotchas)

- **飘血字纯视觉，绝不动 HP。** Crystal `GameScene.DamageIndicator`（`Crystal/Client/MirScenes/GameScene.cs:3511`）也只把一个 `Damage` 推进 `obj.Damages`；HP 走 `ObjectHealth`。若想从飘血字反推血量——别。
- **颜色语义来自 Crystal**：Hit = 怪物白 / 玩家红，Miss = 浅灰/浅珊瑚，Crit = 暗红（`GameScene.cs:3521-3530`）。`pushDamageFloater`(:9207) + `.scene-damage-floater` CSS(:585) 镜像它；`heal`(正 delta) 是本端新增的绿色 variant，Crystal 无。
- **`damageType` 是数字枚举 `0=Hit 1=Miss 2=Critical`**（`CrystalDamageType`，events.ts:24）。`pushDamageFloater` 里 `damage > 0` 才判 heal，否则 `Math.abs(damage)`——别把 miss(damage 常为 0) 当 0 伤害 hit。
- **闪光靠 `motionNow` 单调时钟，不是 `Date.now()` 直读**。overlay 是无 timer 设计：组件只在每个 render tick 用 `motionNow - struckAt` 算年龄，过期自然 return null。隐藏 tab → rAF 暂停 → `motionNow` 不前进，反馈会「冻」(见 MEMORY `mir2-chrome-mcp-verify-gotchas`)。
- **闪光是「兜底」打击感**，因为很多怪 atlas 的 struck 帧被截断（`actor-sprite-lib-truncation`），sprite 自己可能不闪。overlay 闪光与 renderer 无关，所以一定看得见。
- **怪物音效 id = `BaseImage*10 + offset`**（attack +1 / swing +4 / die +3，`original-sound-triggers.ts:18-25`，源 `MonsterObject.cs`）。`BaseImage` 从 `bodyLibrary` 串解析（`"Monster/042"`→42）。没有 SoundList 条目的怪**就是静音**——和 Crystal 一致，不是 bug。
- **玩家近战 swing 固定用 `swingSword`**：武器类别客户端没跟踪（`playEntityAttackSound` :77 注释）。
- **`DamageIndicator` 走 gateway 的 `other =>` 通用 arm**，不是手写 arm。给它加/改字段时，camelCase key 由 protocol 上的 `#[serde(rename_all_fields="camelCase")]`（packets.rs:1916）自动产生——page.tsx 读的是 `payload.objectId`/`payload.damageType`，别假设 snake_case。
- **AoE 风暴不会撑爆 overlay**：`pushDamageFloater` cap 在 48（:9240）；Crystal 是 per-object cap 10（`GameScene.cs:3517`）。两者口径不同但都防爆。
- **图集优先**：只要 `loadEffectAssets()` 能 `resolveSpellEffect`/`resolveMapEffectByNumber` 命中，采集器就**跳过**该特效的 CSS fallback（vfx-fallback.ts:756/797），让真·帧动画权威。本地 manifest 缺失/spell 无条目时才走 `FallbackVfxNode`。

### 程序化特效参考 (FallbackVfx kind / 几何 / 时长)

`FallbackVfx` 在 **tile-delta 空间**（相对 render player，和 sprite/projectile 同坐标基），跟摄像机 pan。`FallbackVfxNode`（visual-layers:236）只有两种几何：

- **`streak`**（投射物轨迹）：从 `(dx,dy)` 到 `(toDx,toDy)` 画一根旋转的细条（`atan2` 求角，长度按 `style.progress` 增长，visual-layers:254）。
- **其余所有 kind**：以 tile 中心的发光环/爆点（`aura` 是空心环，其它实心 radial-gradient burst，visual-layers:286）。新 kind 不必改 `FallbackVfxNode`——只要在 `fallbackVfxStyle` 加 easing，默认走 burst 分支即可安全渲染。

每个 kind 的「手感」= `fallbackVfxStyle`（vfx-fallback.ts:494）里一段独立 easing（opacity/scale 包络），时长是 `vfx-fallback.ts:579-587` 的常量：

| kind | 含义 | easing 特征 (fallbackVfxStyle) | 典型时长 ms |
|---|---|---|---|
| `cast` | 施法蓄力 | 快涨→慢落（:502）| 520 |
| `streak`/`chain` | 投射/电链轨迹 | 全程亮、末段淡（:561）| 投射物寿命 |
| `impact` | 单体命中爆 | 急 pop→淡出（:515）| 440 |
| `nova` | 范围冲击波 | 瞬间铺开→变薄（:521）| 620 |
| `aura` | 持续护盾/光环 | 扩张环 + 渐淡（:509）| 1100 |
| `cloud` | 毒/腐蚀云 | 升起→停留→消散（:533）| 1500 |
| `heal` | 治疗光 | 双脉冲暖光（:547）| 900 |
| `flash` | 瞬移/闪现 | 瞬白→塌缩（:527）| 360 |
| `summon` | 地面召唤喷发 | 上喷→落定（:541）| 700 |
| `curse` | 诅咒内吸 | 起宽暗→向内收（:555）| 760 |

archetype→kind 的映射在 `instantKindForArchetype`(:591)（cast/instant）与 `impactKindForArchetype`(:623)（投射物落地）。

## 如何扩展 (How to extend / add to this area)

遵循「additive / optional / 不破坏 `DisplayWorld` 既有消费者」规则。

**A. 给飘血字加一个新 variant（如「格挡 Block」）：**
1. `lib/game-events/events.ts`：若需要新的 `damageType` 语义，扩 `CrystalDamageType`（或新增事件变体）——保持向后兼容。
2. `app/components/original-client-types.ts:131`：往 `DisplayDamageFloater.variant` 联合类型加 `"block"`。
3. `app/page.tsx` `pushDamageFloater`(:9207)：加一条 `else if` 算出该 variant 的 `text` + `durationMs`。
4. `app/components/original-client-scene-overlays.tsx` `OVERLAY_STYLES`(:571)：加 `.scene-damage-floater.variant-block { color: … }`（内联样式，**不**碰 globals.css）。
5. 核对 Crystal 颜色：`Crystal/Client/MirScenes/GameScene.cs:3519` 的 `switch (p.Type)`。

**B. 新增一个由 server packet 触发的反馈（声 + 画）：**
1. `lib/game-events/events.ts`：加一个 `XxxEvent` 变体并并入 `GameEvent` union。
2. `lib/game-events/sound-subscriber.ts`（:63）和/或 `vfx-subscriber.ts`（:52）：`bus.on("xxx", …)` 接到对应 sink 方法；如需新 sink 方法，扩 `AudioSink`/`VfxSink` 接口 + `makeRealAudioSink`(:141)。
3. `app/page.tsx` 对应 `case "Xxx"`：只 `gameBusRef.current!.emit({ type:"xxx", … })`，**不**直接调播放/渲染——保持总线解耦。
4. 若是新 DOM 视觉：在 `original-client-scene-overlays.tsx` 加一个子组件（仿 `HitFlashes`），用 `entityOriginScreenPosition`(:49) 定位以跟随 sprite，在 `OriginalClientSceneOverlays`(:656) 里挂上，再在 `app/original-client-shell.tsx:2175` 透传新 props。

**C. 给某个 spell 调 fallback 观感（图集到位前）：**
1. `lib/vfx-fallback.ts` `SPELL_TABLE`(:221)：按 Crystal `Spell` enum id（`packages/protocol types.rs`）加/改 `{ id, name, element, archetype }`。
2. 需要新视觉形态时，扩 `FallbackVfxKind`(:428) + 在 `fallbackVfxStyle`(:494) 加一条 `case` 的 easing，并在 `instantKindForArchetype`(:591)/`impactKindForArchetype`(:623) 把 archetype 映射过去。新 kind 默认走 `FallbackVfxNode` 的中心 burst 分支，自动安全渲染。
3. **真·图集**永远优先于此——长期修复是跑 `scripts/export-crystal-magic-effects.mjs` 汇编 manifest（见 `docs/client/audio-vfx.md`），fallback 只是缺资产时的优雅降级。

## 相关 (Related)

- `docs/client/audio-vfx.md` — 音频解析引擎（id→wav、prod allowlist、BGM 单例）+ 法术真·图集汇编管线（姊妹文档，深入处不重复）。
- `docs/client/stage5-social.md`、`docs/client/inventory.md` — 其它客户端区块。
- 源码：`app/components/original-client-scene-overlays.tsx`（本区主渲染）、`lib/vfx-fallback.ts`、`lib/original-sound-triggers.ts`、`lib/game-events/{events,vfx-subscriber,sound-subscriber}.ts`、`app/page.tsx`（`DamageIndicator` 等 case + `pushDamageFloater`/`markEntityStruckFlash`）。
- Crystal 权威：`Crystal/Client/MirScenes/GameScene.cs:3511`（`DamageIndicator`）、`Crystal/Client/MirObjects/Damage.cs`（浮标本体 + Draw）、`Crystal/Client/MirObjects/MonsterObject.cs` / `PlayerObject.cs`（动作音效公式）。
