# 音频 + 法术特效图集 — client map

> 客户端「前置铺垫」文档之一。索引与「如何加功能」配方见 apps/web/CLAUDE.md。

## 这块是干什么的 (What it does)

两条独立的「呈现」管线，都从 page.tsx 的 packet 处理流出，且都对缺资产做**优雅降级**：

1. **音频 (audio)** — 把 Crystal SoundList 的 sound id 解析成一个 `.wav` 路径并播放：背景音乐 (BGM) 走单例 `<Audio>` 循环，战斗/UI 音效走一次性 `<Audio>`（最多 8 个并发）。绝大多数战斗/移动音效是 **客户端自己** 按 Crystal 公式算 id 触发的（不是服务器推的），镜像 Crystal 的 `MonsterObject.cs` / `PlayerObject.cs`。
2. **法术特效图集 (magic-effect atlas)** — `loadEffectAssets()` 拉一个由汇编脚本生成的 manifest（`effects.generated.json`），把 spell 名/数字解析成真实 Crystal 帧动画。manifest 缺失或某 spell 没条目时，回退到一套**纯 CSS 程序化特效** (`vfx-fallback.ts`)，所以施法/技能始终可见。

Audio resolution and the magic atlas are sibling "asset-or-graceful-fallback" pipelines; both fan out from page.tsx packet handlers via the `gameBusRef` event bus.

## 入口在哪 (Entry points)

| 文件 File | 作用 Role | 关键符号 Key symbols (file:line) |
|---|---|---|
| `lib/original-audio.ts` | 播放引擎：BGM 单例循环 + 一次性 SFX 池 + 音量/开关设置 + miss 诊断 | `setOriginalMusic` :43, `setOriginalMusicId` :76, `playOriginalSoundPath` :93, `playOriginalSoundId` :117, `playOriginalSoundIdWithFallback` :128, `playOriginalSoundEvent` :142, `recordMissingSound` :168 |
| `lib/original-sound-index.ts` | id→wav 解析 + **生产允许表 (prod allowlist)**：只返回真正提交/上线的 wav 路径 | `crystalSoundPath` :46, `publicPathIsPresent` :30, `crystalSoundIsMissingAsset` :60, `presentSoundFiles` :26 |
| `lib/original-sound-events.ts` | 语义化 sound id 常量表 + 每事件的有序 fallback 链 | `ORIGINAL_SOUND_IDS` :12, `ORIGINAL_SOUND_EVENT_FALLBACKS` :92 |
| `lib/original-sound-triggers.ts` | Crystal 客户端按动作算 id 的公式（怪物 roar / 玩家 swing / 性别死亡音…） | `playEntityAttackSound` :63, `playEntityStruckSound` :82, `playEntityDieSound` :98, `playMagicSoundId` :117, `monsterImageFromBodyLibrary` :45 |
| `lib/game-events/sound-subscriber.ts` | 把 GameEvent 总线事件接到上面的播放器（注入式 `AudioSink`，可测） | `registerSoundSubscriber` :63, `makeRealAudioSink` :141 |
| `lib/crystal-magic-effects.ts` | 特效 manifest + 各 lib 的 `meta.json` 解析；spell/数字→可播放动画 | `loadEffectAssets` :78, `resolveSpellEffect` :203, `resolveMapEffectByNumber` :363, `effectNameForNumber` :376, `SPELL_NAME_BY_ID` :225 |
| `lib/vfx-fallback.ts` | 无图集时的程序化 CSS 特效（element×archetype 分类 + 每帧样式 + 采集器） | `classifySpellName` :401, `collectViewportFallbackVfx` :846, `fallbackVfxStyle` :494, `SPELL_TABLE` :221 |
| `app/components/original-client-scene-visual-layers.tsx` | 渲染层：拉图集、采集 fallback、画 `FallbackVfxNode` | `useEffectAssets` :218, `loadEffectAssetsOnce` :201, `FallbackVfxNode` :236, fallback 采集调用 :369 |
| `app/components/original-client-shell-flow.ts` | 登录/选人 BGM 常量 + 每屏 BGM 选择（**路径式**，非 id 式） | `ORIGINAL_AUDIO` :6, `desiredMusicForScreen` :33 |
| `app/components/original-client-overlays.tsx` | `SpriteButton`（按钮点击音）| `SpriteButton` :738，点击音 `playOriginalSoundId(10100)` :762 |
| `scripts/export-crystal-magic-effects.mjs` | 汇编 `effects.generated.json` + 各 lib `meta.json`（从 R2 meta 或 `.Lib`） | `SPELL_EFFECTS` :52, `assembleMagicEffectsFromMeta` :179, `runCrystalMagicEffectExport` :223 |

生成物 (committed): `public/original-effects/effects.generated.json`（本分支已是真实汇编结果，含 `Magic/Magic2/Magic3`），`public/original-ui/sound-index.generated.json`（450 条 id→wav，全量导出），`lib/generated/crystal-present-sounds.generated.json`（本地实际存在的 ~320 个 wav）。

## 数据流 (How it threads the 5 layers)

**音频（入站，server-driven `PlaySound`）**
```
ServerPacket::PlaySound
  └─ gateway server_packet_to_event → JSON { sound }
     └─ page.tsx case → gameBusRef.current!.emit({ type:"playSound", soundId }) (page.tsx:6948)
        └─ sound-subscriber bus.on("playSound") → audio.playOriginalSoundId(soundId)
           └─ original-audio.playOriginalSoundId → crystalSoundPath(id) → playOriginalSoundPath → new Audio().play()
```

**音频（客户端自算，最常见的一类）** — 战斗/移动音不靠服务器推 id，而是 page.tsx 在收到 `ObjectAttack`/`ObjectStruck`/`ObjectDied`/`ObjectMagic` 时 emit 语义事件，subscriber 调 `original-sound-triggers.ts` 用 Crystal 公式算 id：
```
page.tsx markWorldEntity… 
  emit "entityAttack"/"entityStruck"/"entityDied" {objectId}   (page.tsx:6865 / 9275 / 9348)
  emit "magicCast" {objectId, spell}                            (page.tsx:9149)
    └─ sound-subscriber → audio.soundEntityRefFor(objectId)（从世界态查 SoundEntityRef）
       └─ playEntityAttackSound / playEntityStruckSound / playEntityDieSound / playMagicSoundId
          • 怪物 roar = BaseImage*10 + offset（attack+1 / swing+4 / die+3），来自 bodyLibrary "Monster/NNN"
          • 玩家 swing/struck/die = SoundList 武器常量（恒存在），死亡按 genderKey 选 male/female
```

**BGM（两套，注意区别）**
- 登录/选人屏：**路径式**。`shell` 用 `desiredMusicForScreen(screen)` 选 `ORIGINAL_AUDIO.loginMusic`/`selectMusic` 直接路径 → `setOriginalMusic(src)`（shell:560）。
- 进游戏后地图音乐：**id 式**。`MapInformation`/进图 → page.tsx emit `{ type:"mapMusicChanged", musicId }`（page.tsx:6583）→ subscriber `setOriginalMusicId(id)` → `crystalSoundPath(id)` → 循环。

**法术特效（入站施法 + 程序化回退）** — 注意：**没有专门的特效 packet 链**。特效从已有的世界态快照（正在 range-attack 的实体、在途 projectile）**被动派生**：
```
viewportEntitySprites / viewportProjectiles (page.tsx 已算好的 tile-delta 数据)
  └─ scene-visual-layers: effectAssets = useEffectAssets()（拉 effects.generated.json + 各 lib meta.json）
     └─ collectViewportFallbackVfx({entities, projectiles}, {now, assets})   (:369)
        • 对每个施法/projectile：若 resolveSpellEffect(assets, spell) 命中真图集 → 跳过 fallback（图集优先）
        • 否则按 spell 名/数字 classifySpellName → element + archetype → FallbackVfx 描述子
     └─ <FallbackVfxNode> 用 fallbackVfxStyle(effect, now) 每帧画内联样式 div（:560）
```
> 注：`MapEffect` packet 今天只在 page.tsx 记 log（page.tsx:6893），`ObjectEffect` 只 `restoreObjectSelection`（page.tsx:8089）——两者都**不**派生真特效；`collectMapEffectFallbacks`（vfx-fallback.ts:665）已就绪但调用方需显式传 spawns（尚未接线）。

## 状态形状 (State shape)

这块**几乎不写 `world.*` / `world.stage5Systems.*`**——它是命令式播放 + 从已有视口数据派生，不存自己的 React 态。

- **音频**（全部模块级单例，不在 React 里）：`musicAudio: HTMLAudioElement|null`、`activeMusicSrc`、`pendingMusicSrc`、`activeEffects: Set<HTMLAudioElement>`（≤8）、`audioSettings: OriginalAudioSettings`（`musicEnabled/effectsEnabled/musicVolume/effectsVolume`，持久化到 localStorage `mir2.originalAudioSettings`）。miss 诊断挂在 `window.__mir2AudioDiagnostics`（`original-audio.ts:177`）。
- **特效图集**：`effectAssetsPromise`（模块级，只拉一次，`scene-visual-layers.tsx:200`）→ `useEffectAssets()` 的本地 state `assets: EffectAssets|null`。`EffectAssets` = `{available:Set, libraries:Map<lib,LibraryMeta>, spellByName, mapByName, groundBySpell, effectNameByNumber}`（`crystal-magic-effects.ts:56`）。
- **fallback 描述子**：`FallbackVfx[]`（每帧由采集器算出的瞬时数组，不入态；`vfx-fallback.ts:447`）。
- **总线**：`gameBusRef = useRef<GameEventBus>`（page.tsx:1332），施法时另维护 spell 名映射（采集器读 `spellByCaster?: Map<objectId|projectileKey, spellName>`，`vfx-fallback.ts:740`）。
- **从世界态读的输入**：`SoundEntityRef = { kind, sprite.bodyLibrary, genderKey }`（`original-sound-triggers.ts:30`）由 `soundEntityRefFor(objectId)` 从实体态投影；`mapMusicChanged.musicId` 来自地图 payload 的 `music` 字段。

## 坑 & 不变量 (Invariants & gotchas)

- **生产允许表 (prod allowlist) 是核心坑**。`crystalSoundPath`（`original-sound-index.ts:46`）只在 id 既在 `sound-index`（450 条）**又**在 present-sounds manifest（本地 ~320 个 wav）里时才返回路径，否则 `null`→静默跳过 + 记 miss。这是为了不去播会 404 的 bytes，并让诊断诚实。生产环境 wav 由 SW 从 R2 回填（`NEXT_PUBLIC_MIR2_ASSET_BASE_URL` 开启）。**不要**为「本地 404」去裁剪 `generate-present-sounds.mjs`——那会把 manifest 从 320 砍到 4，静音生产 ~316 个音（见 memory「present-sound manifest is a prod allowlist」）。
- **每首 BGM 一个单例 `musicAudio`**。屏切换时是 `pause()→改 src→play()`，所以**第一次** `play()`/range 请求常以 `AbortError`（status 0）良性中止；HTTP **206**（ranged）= 成功，不止 200。
- **音频需要用户手势解锁**。`unlockOriginalAudio()` 由 shell 在首个 `pointerdown`/`keydown` 触发（shell:568）；在自动化里不喂手势 → `play()=NotAllowedError`，被吞掉看着像静音。Chrome 验收要 `--autoplay-policy=no-user-gesture-required`。
- **按钮音当前播 `10100`**：`SpriteButton`（overlays.tsx:762/771/786）硬编码 `playOriginalSoundId(10100)`，而 `ORIGINAL_SOUND_IDS.uiButtonClick = 10100`（= LoginEffect / 100.wav），`buttonA = 10103`。Crystal 在 `MirControl.OnMouseClick` 按钮各放各的 Sound（多数是 ButtonA），所以「统一一个音」是忠实的；**本分支用的是 10100，不是 ButtonA 10103**（10100→10103 的修正不在此分支）。
- **怪物 roar 可能静音是对的**。`monsterBaseSound` = image*10（image 从 `bodyLibrary` `Monster/0*NNN` 解析），只有该怪有 SoundList 条目才出声——和 Crystal 一致（无条目即静音）。玩家 swing 不区分武器（客户端不跟踪武器类），固定用 `swingSword`（`original-sound-triggers.ts:77`）。
- **magic-cast 音 (`20000 + spell*10`) 几乎都解析不到**。`playMagicSoundId`（:117）按 Crystal 公式算，但绝大多数 20000 段 id 不在当前 SoundList → `null` 优雅跳过；保留是为 parity 完整性。
- **图集优先 (atlas-first)**。`collectViewportCast/ProjectileFallbacks`（vfx-fallback.ts:744/785）会先 `resolveSpellEffect(assets, spell)`；命中真图集就**跳过**程序化 fallback。所以新增真图集会自动抑制对应 fallback，不会双画。
- **施法 = `attackAnimation === "range"`**。采集器靠实体的 `attackAnimation/attackStartedAt` 识别施法（vfx-fallback.ts:751），不靠独立 packet。
- **`effects.generated.json` 是汇编产物，不是 `.Lib` 再导出**。帧 PNG 已在 R2 全量释出（`/original-ui/Magic*/N.png` + `meta.json`）；汇编脚本只写「指向这些已上线帧」的小 manifest。`spell_effect_enum` 当前为空 `[]` → `effectNameForNumber` 回落到内建 `SPELL_NAME_BY_ID`（crystal-magic-effects.ts:377）。
- **本地 vs 生产**：本地 `/original-ui/Magic/*.png` 会 404（资产仅 R2），所以本地施法仍走程序化 fallback；生产 SW 从 R2 回填后才放真帧。spell→base/count/interval 来自 Crystal `PlayerObject.cs case MirAction.Spell:`（见脚本头注释 export-crystal-magic-effects.mjs:19-25），方向性 spell 只导 dir-0 切片。

## 如何扩展 (How to extend / add to this area)

**加一个新的语义化 UI 音效（如某窗口动作音）**
1. `lib/original-sound-events.ts`：在 `ORIGINAL_SOUND_IDS` 加一个具名常量（值 = SoundList id；UI 音惯例 `10000 + 文件号`）；需要兜底就在 `EXPLICIT_FALLBACKS` 加有序链。
2. 确认该 wav 在 present-sounds manifest 里（否则生产前先跑 `npm run generate:present-sounds` 并提交，或确认 R2 有）。
3. 在 page.tsx 对应 case `gameBusRef.current!.emit({ type:"uiSound", event:"<新键>" })`（`OriginalSoundEvent` 是 `ORIGINAL_SOUND_IDS` 的键，类型自动收口）。
4. `npm run qa:audio` 验回路（注意 206/AbortError/手势解锁的坑）。

**加一个服务器推送的新音（新 `PlaySound`-类 packet）**
1. 协议 → gateway `server_packet_to_event` 产出含 `sound` 的 camelCase JSON。
2. page.tsx 新 case → emit `{ type:"playSound", soundId }`（id 解析 + miss 处理已在 `original-audio.ts` 内，无需改播放器）。

**给某 spell 加真 Crystal 特效帧（替掉程序化 fallback）**
1. 在 `Crystal/Client/MirObjects/PlayerObject.cs` 找该 spell 的 `case MirAction.Spell:` 块，读 `Effects.Add(new Effect(Libraries.<Lib>, <base>, <count>, <duration>, ...))`。
2. `scripts/export-crystal-magic-effects.mjs` 的 `SPELL_EFFECTS`（:52）加一行 `{ spell, library, base, count, interval, kind }`（interval = 每帧 ms，按头注释的换算）。
3. 跑 `npm run export:crystal-magic-effects -- --assetBaseUrl <R2 release base>`（assemble-from-R2-meta 模式），它重写 `public/original-effects/effects.generated.json` + 各 lib `meta.json`；提交这些生成物。
4. 无需改渲染层——`resolveSpellEffect` 命中后 `collectViewportCastFallbacks` 自动抑制该 spell 的 CSS fallback。`npm run qa:vfx` 验 ③ 通过（生产需 R2 帧已部署，本地仍 404→procedural）。

**调某 spell 的程序化 fallback 观感（无真图集时）**
1. `lib/vfx-fallback.ts` 的 `SPELL_TABLE`（:221）改/加该 spell 的 `{element, archetype}`；找不到时走 `ELEMENT_KEYWORDS`/`ARCHETYPE_KEYWORDS` 关键词扫描。
2. 需要新视觉「kind」就扩 `FallbackVfxKind`（:428）+ 在 `fallbackVfxStyle`（:494）加 case；渲染层 `FallbackVfxNode`（scene-visual-layers.tsx:236）对未知 kind 已有安全默认（居中径向 burst）。

> 规则：所有新增字段/事件 **可选 + 向后兼容**，绝不破坏 `DisplayWorld` 既有消费者；改前先 `npx tsc --noEmit`（必须 0）。

## 相关 (Related)

- 源码：`apps/web/lib/original-audio.ts`、`original-sound-index.ts`、`original-sound-events.ts`、`original-sound-triggers.ts`、`crystal-magic-effects.ts`、`vfx-fallback.ts`
- 渲染 / 接线：`apps/web/app/components/original-client-scene-visual-layers.tsx`、`lib/game-events/sound-subscriber.ts`、`lib/game-events/vfx-subscriber.ts`、`app/components/original-client-shell-flow.ts`
- 工具：`apps/web/scripts/export-crystal-magic-effects.mjs`、`scripts/crystal-library.mjs`、`scripts/generate-present-sounds.mjs`；验收 `scripts/qa-audio.mjs`（`npm run qa:audio`）、`scripts/qa-vfx.mjs`（`npm run qa:vfx`）
- Crystal 权威：`Crystal/Client/MirObjects/PlayerObject.cs`（施法/玩家音 + `case MirAction.Spell:`）、`Crystal/Client/MirObjects/MonsterObject.cs`（怪物 BaseSound 公式）、`Crystal/Client/MirSounds/SoundList.cs`（id 命名）、`MirControl.OnMouseClick`（按钮音）
- 兄弟文档：见 `docs/client/`（同目录其它「前置铺垫」），索引 `apps/web/CLAUDE.md`
