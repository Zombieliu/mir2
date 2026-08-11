# Mir2 Web3 本地运行 & 调试经验

> 本文沉淀本项目的本地启动、资产、渲染调试经验,供任何 agent(session)在新会话中快速恢复上下文。
> 维护规则:每解决一个"会再遇到"的问题,就在这里补一条。每条都要给出"症状 → 根因 → 解法"。

## 0. 本项目的上下文管理机制(agent 如何"变聪明")

> 任何 agent 新开会话时,通过下面这套分层上下文恢复本项目知识。遇到新问题时,把经验按此机制沉淀回去,形成"越用越聪明"的闭环。

### 三层上下文

| 层 | 载体 | 生命周期 | 作用 |
|---|---|---|---|
| 会话内上下文 | 当前对话消息 | 单个会话,用完即弃 | 当前任务状态 |
| 项目上下文 | `AGENTS.md` → 本文件 + 各 `docs/*.md` | 常驻,每会话自动注入 | 项目级知识(启动/架构/坑) |
| 持久记忆 | `~/.hermes/memories/MEMORY.md`(Hermes) | 跨会话 | 从历史对话自动提炼的经验 |

### 为什么需要这套机制

所有 agent 的上下文窗口有限,新会话从零开始。没有记忆 = 每次重开都重新摸索。这套机制让新会话能"读档续玩"而不是"重开新档"。

### 如何维护(让经验持续累积)

1. **解决会再遇到的问题** → 在本文件按"症状 → 根因 → 解法"补一条,并同步到 Hermes `MEMORY.md`
2. **Hermes 会自动学习**——它在对话中自动把值得记住的经验写进 `~/.hermes/memories/MEMORY.md`(`memory_enabled=true`),无需手动维护
3. **本文件是权威细节**,MEMORY.md 是 Hermes 的"索引式快照"(每条限 2200 字符)

### 已沉淀的关键经验(快速索引)

- §1 本地启动命令与关键环境变量(含 `VERCEL=1` 等陷阱)
- §2 常见坑:gateway lease 残留、gateway 二进制丢失、public 资产缺失
- §3 渲染调试:实体闪烁根因(图集默认关闭)+ 修复、后端判定、AOI 抖动、帧缺失
- §4 调试工具:Playwright 像素检测、登录自动化
- §5 资产加载架构(Starter vs Full Pack vs Bevy 图集)

---

## 1. 本地启动(标准流程)

仓库是标准 Git checkout，主项目位于仓库内的 `mir2-web3/`。开始本地命令前先执行
`cd mir2-web3`；除非特别注明，本文后续的 `apps/`、`scripts/`、`packages/` 和
`docs/` 路径均相对该主项目目录。

### 一键启动脚本 `scripts/start-local.sh`

```bash
./scripts/start-local.sh          # 启动 gateway + web
./scripts/start-local.sh stop     # 停止
./scripts/start-local.sh status   # 健康检查
```

- 服务地址:web `http://127.0.0.1:3002/`,gateway health `http://127.0.0.1:7110/health`,WebSocket `ws://127.0.0.1:7110/ws`
- 登录 demo 账号:**account=`demo` / password=`demo`**(QA 脚本确认)
- 脚本已内置关键环境变量(见下),**不要手动 export + nohup**(env 会丢,导致 r2-proxy 等失效)

### 关键环境变量(start-local.sh 已处理)

| 变量 | 值 | 作用 |
|---|---|---|
| `VERCEL=1` + `VERCEL_ENV=production` | 必须 | next.config 只有在此条件下才读取 `config/production-web-assets.json`,启用 full pack 资产 |
| `NEXT_PUBLIC_MIR2_ASSET_BASE_URL` | `https://assets.mir2.obelisk.build/mir2/v/20260730-fullcrystal-f71b89aa-gzip1` | 资产 CDN,启用远程回退 |
| `MIR2_R2_PROXY_BASE` | 同上 | 启用 r2-proxy(本地缺失资产从 CDN 代理) |
| `MIR2_ORIGINAL_ASSET_MANIFEST_MODE=remote-release` | 必须 | 用 remote map-atlas 而非本地生成 |
| `MIR2_ALLOW_DEV_IDENTITY_SECRETS=1` | gateway 需要 | 开发身份密钥开关 |
| `MIR2_GATEWAY_WEB_ADDR=127.0.0.1:7110` | gateway 需要 | 前端默认连 7110(不是 7010!) |

## 2. 常见坑

### 2.1 Gateway route lease 残留(调试期高频)

**症状**:Start Game 点了没反应,Bevy 不启动。gateway 日志:
```
web StartGame route lease rejected for demo/0: ... held by gateway-<pid>-... until <future timestamp>
```

**根因**:gateway 重启后,旧进程持有的 session route lease 不释放,新 gateway 因 owner 不匹配拒绝。每次重试还会续租,导致 demo 永久被锁。

**解法**:
```bash
pkill -9 -f mir2-gateway   # 彻底杀干净
rm -f /tmp/gateway.log     # 清掉污染日志
# 再重新启动
```
lease 在**内存**,彻底杀进程即清。新账号不受影响(绕过用注册新账号)。

### 2.2 Gateway 二进制可能丢失

**症状**:`./target/debug/mir2-gateway` 不存在。

**解法**:重新构建 `cargo +1.89.0 build --locked -p mir2-gateway`(约 1-2 分钟)。

### 2.3 `public/` 资产缺失(曾发生)

**根因**:`apps/web/public` 有 15283 个 git 追踪文件(含 `mir2-asset-worker.js`),但可能因恢复不全缺失。

**解法**:从镜像仓库 `~/obelisk/ai/numeron/mir2` 用并发 `git cat-file` 恢复,注意**含空格路径**(如 `AWeapon/00 L/`)需用 `-z` + python 处理。

### 2.4 本地开发从旧 R2 加载新 Bevy runtime 导致 404

**症状**:本地 Next 开发环境的地图可能退回兼容渲染，Network 中出现类似以下 404：

```text
https://assets.mir2.obelisk.build/mir2/v/<旧版本>/bevy-runtime/v/<当前版本>/pkg-webgpu/mir2_bevy_runtime.js
```

**根因**:`NEXT_PUBLIC_MIR2_ASSET_BASE_URL` 是完整 Crystal 资产包的通用 R2 基址；本地刚构建的
`bevy_runtime_version.json` 可能尚未上传到这个不可变 release。开发环境若把同一个基址也套到
Bevy runtime，就会请求一个实际上不存在的版本目录。

**解法**:非 production 默认从 Next 同源 `/bevy-runtime/...` 加载当前 runtime；production 继续走
不可变 R2。只有确实准备了单独 runtime CDN 时，才设置
`NEXT_PUBLIC_MIR2_BEVY_RUNTIME_ASSET_BASE_URL`。不要为了消除本地 404 修改或覆盖旧 R2 release。

### 2.5 Crystal 原始本地化目录缺失时禁止全量重写生成包

**症状**:运行 `packages/tooling/scripts/import-crystal-localization.mjs` 时找不到相邻
`Crystal/Client/Localization` 或 `Crystal/Build/Server/Debug/Localization`，若仍从空对象生成，会把已提交
的数千条 `client.*` / `server.*` 文案删除，只剩项目自定义键。

**根因**:当前工作树可能是稀疏开发包，不保证携带 Crystal 原始仓库；生成器不能把“原始输入不存在”误判为
“原始输入为空”。

**解法**:原始 JSON 缺失时，以已提交的
`packages/game-data/data/generated/localization_bundle.json` 作为只读基线，只补充缺少的生成键并保留既有
顺序和元数据。生成后必须再次运行生成器并比较哈希，确认输出幂等；同时用 `cmp` 确认 Web 副本与包副本一致。

## 3. 渲染调试(核心经验)

### 3.1 实体闪烁(已解决——最重要)

**症状**:带动画的怪物/NPC/角色在真实浏览器(GPU)上闪烁;headless/swiftshader 测不到。

**根因**:`shouldUseBevyEntityAtlas()` 默认返回 false → Bevy 用"单图 path 加载"渲染实体。真实 GPU 上图集外实体查不到 rect,逐帧 path 加载竞争 → 闪烁。

**解法**(已修复):默认启用 Bevy 动态实体图集。图集 key 稳定不重建。
- 验证参数:`?bevyAtlas=1`(启用)、`?bevyAtlas=0`(禁用逃生)
- 检查状态:浏览器 console `__mir2BevyEntityRendererDebug.atlasMode` 应为 `"packed"`
- 关键文件:`apps/web/app/original-client-shell.tsx` 的 `shouldUseBevyEntityAtlas()`

### 3.2 渲染后端判定

- `window.__mir2BevyRuntimeDebug.selectedBackend` = `"webgpu"` / `"webgl2"` / null
- Chrome 真 GPU 通常 webgpu;headless swiftshader 可能 webgl2 或 webgpu
- 参数:`?bevyBackend=webgpu|webgl2` 强制

### 3.3 DOM 模式(Bevy 全禁)验证

`?bevyEntities=0&bevyCanvas=0&bevyRuntime=0` = 纯 DOM(实体+地图都禁 Bevy)。
`?bevyEntities=0` = 只禁 Bevy 实体,但**地图也受影响**(架构耦合,bevyMapActive 依赖 useBevyEntityRenderer)。

### 3.4 动画帧缺失(已修复)

实体库 meta.json 只含 **80 帧**(移动帧),但 frame-set 定义攻击/受击/死亡需 **80+ 帧**。旧逻辑缺失帧回退到第0帧(站立)→ 动画跳变闪烁。已改为**模运算映射**(缺失帧平滑映射到可用帧)。
- 验证:`public/original-ui/Monster/004/meta.json` 的 `frames.length=80` vs `count=232`

### 3.5 AOI 边界实体抖动(已修复)

玩家移动时,服务器 AOI 边界实体从 `world.entities` 周期性消失又出现 → 客户端立即镜像 → 实体挂载/卸载闪烁。
- 已加 **entity grace period**(`displayEntities` 保留 2 秒才移除消失实体)

## 4. 调试工具

### Playwright 像素级检测

项目里有 playwright-core 依赖 + Chromium 缓存:
```bash
CHROMIUM="$HOME/Library/Caches/ms-playwright/chromium-1187/chrome-mac/Chromium.app/Contents/MacOS/Chromium"
PLAYWRIGHT_CHROMIUM="$CHROMIUM" node apps/web/<script>.js
```
脚本:登录 → 创建角色 → Start Game → 采样实体稳定性 / 像素 diff。所有脚本在 `apps/web/*.js`(临时)。

### 登录自动化要点

- 按钮是 `button[aria-label="Login"]` / `button[aria-label="New Account"]` / `button[aria-label="Start Game"]`
- 新建角色:点 `New Character` → 点 `Create`(OK)按钮 → `Start Game`
- 新账号密码用 `Mir2test1`;demo 账号密码 `demo`

## 5. 资产加载架构

- **Starter 资产**(public/ 15283 文件):git 追踪,本地有,登录/新手用
- **Full Crystal Pack**(`generated/crystal-packs/full/`,9GB+):不在 git,**R2 CDN 远程按需加载**
- **Bevy 动态实体图集**:从当前场景实体帧动态打包,GPU 渲染
- 资产加载设计参考:`docs/ASSET-CONSUMER-SETUP.md`(三种模式)

## 6. 跨平台客户端（main 集成基线）

### 6.1 Crate 布局与工具链

- `apps/game-client/client-core`:平台无关表现数学,**零外部依赖**;统一用 `cargo +1.95.0 test`(12/12)
- `apps/game-client/runtime`:现有 Bevy WASM 运行时,消费 client-core;crate 内 `rust-toolchain.toml` 是 **1.95.0**(Bevy 0.19 需要),与根目录 1.89.0 不同
- `apps/game-client/platform-windows`:原生 Windows/macOS 桌面宿主(共享 `build_runtime_app` + gateway WS 客户端)
- 跨平台 crate 都是独立 `[workspace]`(不带 server workspace),路径依赖链接

### 6.2 原生编译注意

- **运行时原生编译用 `cargo +1.95.0`**(不是根目录的 1.89.0,否则报 `bevy requires rustc 1.95.0`)
- macOS 原生宿主冒烟:`apps/game-client/platform-windows` 直接 `cargo +1.95.0 run` 开窗口
- **原生宿主连本地 gateway**:gateway 默认 `ws://127.0.0.1:7110/ws`,字符 index 默认 0;账号和密码**没有默认值**,必须显式设置 `MIR2_NATIVE_ACCOUNT` / `MIR2_NATIVE_PASSWORD`;可用 `MIR2_NATIVE_CHARACTER_INDEX` / `MIR2_GATEWAY_WS_URL` 覆盖其余配置。非 loopback 地址必须用 `wss://`
- 确认进图:日志出现 `LoginSuccess` → `StartGame ack` → `forwarded world snapshot #1..3`
- **Windows 交叉编译门**:`apps/game-client/platform-windows/build-windows.sh`
  - 前置:`rustup target add --toolchain 1.95.0 x86_64-pc-windows-gnu` + `brew install mingw-w64`
  - 链接器 `x86_64-w64-mingw32-gcc`;产物 `target/x86_64-pc-windows-gnu/release/mir2-platform-windows.exe`
  - gnullvm 目标(llvm clang+lld)在本机缺 mingw CRT/`libunwind`,不推荐,用 GNU 目标
- **wasm 构建**:`cd apps/web && npm run runtime:build:dev`(构建 webgpu+webgl2 两个后端,产物 gitignore,只有 `apps/web/lib/generated/bevy_runtime_version.json` 提交)

### 6.3 时钟注入(M1-A)

- `runtime/src/motion.rs` 的 `MoveClockSource`:`Wall`(生产,wasm 用 `Date.now()`,原生用 `SystemTime`)/`Manual`(测试冻结)
- 生产宿主保留 `Wall`;确定性测试用 `ManualClock` 注入

### 6.4 Native 摄取通道(M2 gateway)

- `runtime/src/native_ingest.rs`:进程级 std mpsc,后台 gateway 任务跨线程推送快照 JSON,Bevy 主线程每帧 drain
- 各 typed ingest 系统同时 drain thread-local(WASM 路径)+ 原生 channel;WASM 行为不变
- 原生宿主入口:`native_ingest::push_native_world_state(json)` / `push_native_entity_render_state` / `push_native_map_render_state`(任何线程可调,app 构建后可用)
- 新增:`push_native_ui_read_model` / `push_native_map_model` / `push_native_entity_model_set` / `push_native_inventory_model`(client-bevy 各共享 read-model)

### 6.5 client-bevy 共享渲染(read-model 驱动)

`apps/game-client/client-bevy` 消费**渲染器无关 read-model**,Web React 与原生 Bevy 共用,保证值不分叉:

| 模块 | read-model | 渲染 |
|---|---|---|
| `read_model` | `UiReadModel`/`PlayerStats`(HP/MP/gold/level/name) | HUD 条 + 文本 |
| `map` | `MapModel`(terrainPatches+center) | 地形色块(共享占位) |
| `entities` | `EntityModelSet`(kind/x/y/level) | 影子+身体+徽章精灵 |
| `inventory` | `InventoryModel`(gold+items 按 container 分组) | 背包格子 + 金币 |
| `chat` | `ChatModel`(200 行上限) | 聊天面板 |
| `character` | 复用 `UiReadModel` | 角色面板 |

- 数据流:gateway worldSnapshot → platform-windows `transform_*` 提取各模型 JSON → `push_native_*` → runtime ingest → client-bevy resource → 插件渲染
- **占位色块 vs 真实贴图**:`client-bevy::map/entities` 保留跨宿主共享彩色 fallback;Windows 原生宿主已通过 runtime 摄取通道接入真实 Crystal 地图/实体图集(见 §6.6/§6.7)
- **Bevy UI 坑**:`bevy_ui_widgets` feature 的 message 系统未配置即 panic,只用 `bevy_ui`+`bevy_text`;`Text` 需 `TextFont`(`FontSize::Px(...)`)+`TextColor`;`resource_changed` 是系统函数传参不加大括号;两个 `Query<&mut Node>` 冲突需 `ParamSet`

### 6.6 原生真实精灵图集(atlas)

- **实体图集已落地**:`platform-windows/atlas.rs` 加载本地 `apps/web/public/bevy-entity-atlases/starter-bichon-base.png`(4096x4096 / 4.2MB / 2631 帧真实 Crystal 精灵),png crate 解码 RGBA → `push_native_entity_render_atlas` → runtime 共享实体图集存储
- `build_entity_render_state` 把 gateway 实体映射到图集 rect(当前按 kind 选静态帧;完整方向/帧解析是后续)
- **路径坑**:发布包从可执行文件旁的 `mir2-assets` 发现资源;仓库开发模式同时检查 `apps/web/public` 和主仓库布局下的 `mir2-web3/apps/web/public`,不嵌入编译机绝对路径
- **顺序坑**:`push_native_*` 必须在 `build_runtime_app` 之后调用(注册 channel 才生效)

### 6.7 原生真实地图渲染(map parser)

- `platform-windows/map_parser.rs`:**type-100 .map 解析器**(magic `01 43 23` header,width/height @4/6 i16 LE,每格 26 字节)
  - 数据源:打包的 `apps/web/lib/generated/crystal-map-pack/*.map.gz`(flate2 gunzip)
  - `library_key_for_index` 映射 cell middleIndex → `WemadeMir2/Tiles` 等(镜像 Web `mapLibraryKeyForIndex`)
  - tile frame → atlas rect → `MapRenderState` JSON,复用 `generated/map-atlas/manifest.json`
- **图集图片加载路径**:runtime `AssetServer.file_path` 必须是 `apps/web/public`(asset_root 由 `RuntimeWindowSpec.asset_root` 指定),否则 `generated/map-atlas/...` 解析不到
- 验证:Bichon `0.map`(700x700 type-100)真实渲染,实体图集同开,无 asset 加载错误
- 地图图块 rect 目前只做 middle 层静态帧;完整 front/animation/投影是后续
- **front 层/动画帧已补**:`resolve_map_tile_draws` 输出 middle+front 双层(z 0/1),`frame_count` 携带动画宽度;front/middle 动画计数与 additive 解码镜像 `crystal-map-blend.ts`
- **实体方向帧已补**:`starter_frame` = `frameBaseOffset + directionStride * directionIndex`(8 向,镜像 `entity_animation.rs`);manifest rect 按真实每帧尺寸后缀精确匹配(如 `/original-ui/Monster/000/24.png|104x100`)

### 6.8 移动真机门

- `apps/mir2-mobile/build-mobile-device.sh`:构建 APK → 启动 Pixel_5_API_31 headless 模拟器 → 安装 → 启动 `com.obelisklabs.mir2` → 验证进程;iOS sim 构建
- **历史 smoke**:原分支曾完成模拟器 boot → APK 安装/启动 → 进程存活;main-based PR 的 hosted gate 重新构建 APK,但不把该历史 smoke 当成最新 head 的视觉或真机 Accepted 证据
- `capacitor.config.js` 从同一个 `MIR2_MOBILE_GAME_URL` 生成 `server.allowNavigation`,保证 HTTPS 游戏源留在 app WebView;契约测试覆盖默认/自定义 host。最新 head 的模拟器截图、物理真机和人工 UI 确认仍需单独验收
- 真机:`MIR2_ANDROID_SERIAL=XXXX` 指定设备绕过模拟器;iOS 生命周期由 Capacitor 框架处理(launch storyboard + 双方向)

### 6.9 Windows / Android 交叉编译门

- **Windows**:`apps/game-client/platform-windows/build-windows.sh`
  - `rustup target add --toolchain 1.95.0 x86_64-pc-windows-gnu` + `brew install mingw-w64`
  - 链接器 `x86_64-w64-mingw32-gcc`,产物 `target/x86_64-pc-windows-gnu/release/mir2-platform-windows.exe`
- **Android**:`apps/game-client/platform-android/build-android.sh`
  - `rustup target add --toolchain 1.95.0 aarch64-linux-android` + Android NDK(默认发现 `~/Library/Android/sdk/ndk/*`)
  - 本机 NDK 是 darwin-x86_64,ARM mac 下经 Rosetta 运行 clang,可用
  - **关键坑**:bevy 依赖必须启用 `android-native-activity` feature(runtime/Cargo.toml),否则 `android-activity 0.6.1` 缺 `activity_impl` 模块编译失败;该 feature 仅 Android 目标生效
- **wasi/native 回归**:改 runtime Cargo.toml 后要同时跑 `cargo +1.95.0 test`(native)、两个 gate、`npm run runtime:build:dev`(wasm)

## 7. 五平台外壳（main 集成基线）

一套 `apps/web` 客户端,五个平台 WebView 壳 + 原生 Bevy 窗口:

| 平台 | 壳 | 目录 | 门脚本 |
|---|---|---|---|
| Windows/macOS/Linux 桌面 | Tauri 2 | `apps/mir2-launcher-tauri` | `build-desktop.sh` |
| Android/iOS 移动 | Capacitor 7 | `apps/mir2-mobile` | `build-mobile.sh` |
| 桌面原生 Bevy 窗口 | 共享 runtime | `apps/game-client/platform-windows` | `build-windows.sh` |

### 7.1 Tauri 桌面壳

- 发布模式直接导航到稳定 HTTPS 游戏源 `https://mir2.obelisk.build`;dev 模式默认 `http://127.0.0.1:3002`,也可用 `MIR2_DESKTOP_GAME_URL` 显式覆盖
- 壳只负责窗口和导航,不启动本地 Node/standalone server,不依赖源码 checkout;远程页面不获得 shell capability
- **macOS 构建**:`npx tauri build --bundles app`(.dmg 需要 create-dmg,本机缺,跳过)
- **Windows**:`build-desktop.sh windows`(需 mingw-w64 + GNU target)
- **Linux**:需 WebKitGTK 系统库,交叉编译要在 Linux 主机/CI 跑

### 7.2 Capacitor 移动壳

- WebView 加载**远程部署的 HTTPS 游戏 URL**(原生 PWA app),`MIR2_MOBILE_GAME_URL` 配置;gateway WS 通过 query 注入。`capacitor.config.js` 会把相同 build-time URL 的 hostname 写入 `server.allowNavigation`,否则 Capacitor 会把顶层跳转交给系统浏览器
- **Android 构建三坑**(已踩):
  1. **JDK 21 必须**(Capacitor 7 编译 source 21,JDK17 报"无效的源发行版:21")
  2. **gradle 8.11.1 下载慢** → 用腾讯镜像 `https://mirrors.cloud.tencent.com/gradle/gradle-8.11.1-all.zip` 直接下到 `~/.gradle/wrapper/dists/gradle-8.11.1-all/<hash>/` 并解压,~140x 加速
  3. SDK 35 + AGP 8.7.2;`export JAVA_HOME=...jdk21` 后 `./gradlew assembleDebug`
- **iOS 构建**:`cocoapods >= 1.17`(修复 Ruby 3.4 `UnicodeNormalize` ASCII-8BIT bug;brew 1.16.2 有坑,`gem install cocoapods -v 1.17.0 --user-install`);`pod install` 在 `ios/App/` 跑;`xcodebuild -sdk iphonesimulator CODE_SIGNING_ALLOWED=NO`
- 首次 `cap add android/ios` 后提交原生配置(android/ + ios/),build 产物 gitignore
