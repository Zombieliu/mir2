# Mac 接手 Android 开发：分支、上下文与验收

更新：2026-09-03。本文是用户要求的交接，不是 Android 已实现或真机已验收的声明。
Windows 主线仍在修复 Crystal 原版的 UI / 状态 / 交互差异；Android 是独立工作流。

## 1. 同步点与安全分支策略

- 仓库：`Zombieliu/mir2`；主项目在仓库根目录的 `mir2-web3`。
- 来源分支：`codex/windows-player-journey`。
- 现有 Windows PR：[#250](https://github.com/Zombieliu/mir2/pull/250)，仍为 Draft，
  基于 `codex/windows-visual-parity`。不要合并、改 base 或在 Mac 上直接覆盖其分支。
- 本次 Windows 地图修复提交：`63b80fda3cdb4fda54fa4b1867c1c140b9e6db01`。
- 本文随其后的文档提交发布；交接聊天提供包含本文的精确 tip SHA。
- Mac 建议工作分支：`codex/android-player-journey`，从已核验的来源 tip 创建。
  后续 Android PR 可暂以 Windows 来源分支为 base；上游合并后另行核对，不能擅自改栈。

先定位 Mac 上真实的仓库根目录，不照抄 Windows 盘符。读取本地改动后再切分支：

```bash
git status --short --branch
git remote get-url origin
git fetch origin codex/windows-player-journey
git log -3 --oneline origin/codex/windows-player-journey
git merge-base --is-ancestor 63b80fda3cdb4fda54fa4b1867c1c140b9e6db01 origin/codex/windows-player-journey
```

还必须核验聊天中的完整交接 SHA 是该远程分支的祖先或 tip；修复提交本身不含本文。
工作区干净、来源正确、且目标本地分支不存在时，才执行：

```bash
git switch -c codex/android-player-journey origin/codex/windows-player-journey
```

若已有同名分支、未提交修改或分叉，先检查并保留；不得自动 reset、clean、force push、
删除分支、覆盖文件或把用户工作自动 stash。记录实际起点 SHA。不要凭 Windows PR 的
状态宣称 Mac 工作区已经同步成功。

## 2. 必须继承的上下文

先读根目录 `AGENTS.md`，再读以下主项目内路径：

1. `docs/AGENT-ORCHESTRATION.md`
2. `docs/AGENT-TASK-QUEUE.md`
3. `docs/CRYSTAL-1TO1-ROADMAP.md`
4. `docs/BACKEND-1TO1-PROGRESS.md`
5. `docs/CRYSTAL-SERVER-PARITY.md`
6. 本文、`docs/PLATFORM-CLIENT-STRATEGY.md`、`docs/LOW-END-ANDROID-SUPPORT.md`
7. `docs/ASSET-CONSUMER-SETUP.md`、`docs/DEVELOPER-HANDOFF.md`
8. `apps/game-client/platform-android/README.md`、`docs/FRONTEND-1TO1-GAPS.md`

最新交接优先于文档中的历史百分比。当前 `visualAccepted=false`、`accepted=false`、
`globalParityPercent=null`；Windows 的 33 个用户观察问题以及原版对照、DPI、长时间运行、
人眼/手感、资源权利与正式签名等门槛仍未整体关闭。不要把编译、单元测试或某个窗口的
完成冒充整端、整款游戏完成。

### Windows 地图黑底修复已做什么

用户画面中人物、特效、HUD 存在，地面/建筑消失。根因是可执行文件迁移后，资源目录
junction 的 `../lib` 查找落到安装位置，而没有先解析真实资源目录。

- 地图查找器保留打包目录优先级，然后解析真实目录再寻找开发环境的地图布局。
- 启动必须找到并解码比奇 `0.map.gz`，缺失时明确失败，不继续显示错误的完整资源状态。
- 用户 `(302,634)` 视口解析出 607 个 atlas 绘制项、242 个独立绘制项；221 个本地图片
  引用全部存在。Windows 测试 537/537 串行通过，离线构建通过。
- 原始默认并行运行有三个 GameShop 队列隔离相关失败，未修复；不得隐藏或把串行通过
  描述为默认并行也通过。详细证据见
  `docs/generated/player-qa/native-ui-parity-20260903-map-relocation/README.md`。
- 修复 EXE 只观察到登录页；登录后的实际世界画面仍待人工确认。

此修复没有改资源、账号存档、服务器逻辑；也没有自动接到 Android 原生宿主。
本地完整资源位于 Windows 的 F 盘，通过目录映射供项目使用。**Git 同步不包含那份完整
资源包，也不会把 Windows junction 变成 Mac 上可用的资源目录。** Mac 必须按已有资源
获取/校验文档配置自己的路径或批准的资源服务，不写死盘符、不反复下载整个大包、不把
缓存、APK、签名密钥或账户资料提交到 Git。

## 3. 两条 Android 路线的真实现状

| 路线 | 已有内容 | 不能据此声称完成的内容 |
| --- | --- | --- |
| Capacitor / WebView | `apps/mir2-mobile` 已有 Android 工程、加载器、构建脚本和 CI APK 构建步骤；Web 已有移动摇杆、攻击、拾取、快捷技能与面板入口 | 本次没有构建/安装新 APK，没有 Android 真机游戏、性能或后台恢复验收；这不是 Rust 原生 Android 客户端 |
| Rust / Bevy 原生 | `apps/game-client/platform-android` 有生命周期/输入翻译、共享 UI reducer、命令与回执桥、FFI 交接及 check/package 脚本 | README 明确缺实际 Android WebSocket transport 和原生 APK/device 证据；桥接队列不等于可联网游戏，模拟输入测试不等于触屏操作可用 |

关键源码入口：

- Web 壳：`apps/mir2-mobile/scripts/build-web.mjs`、`scripts/capacitor-config.cjs`、
  `android/app/src/main/java/com/obelisklabs/mir2/MainActivity.java`。
- Web 触控：`apps/web/app/components/original-client-mobile-input.ts`、
  `apps/web/app/components/original-client-mobile-controls.tsx`。
- 原生宿主：`apps/game-client/platform-android/src/lib.rs`、`android_input.rs`、
  `gateway_bridge.rs`（后二者也在该 `src` 下）。
- Windows 参考：`apps/game-client/platform-windows/src/main.rs`、`assets.rs`、
  `map_parser.rs`、`gateway.rs`、`input.rs`。
- CI：仓库根目录 `.github/workflows/cross-platform-client.yml`，区分
  Capacitor APK build 与 native Bevy compile，不能混用证据。

当前 Android 没有像 Windows 一样启用 `mir2-client-bevy/native-ui`，其 app builder
也没有直接装配 Windows 的完整地图/实体生产与 Crystal UI 插件。因此原生 UI 不会仅因
拉取相同分支就自动出现；要逐项审计依赖、插件装配和宿主数据路径。

Web 壳当前内置的是加载页，实际打开配置的远程 Web，不是把当前 Git 中的 Next.js
完整应用离线装进 APK。必须显式核对 `MIR2_MOBILE_GAME_URL` 与
`MIR2_GATEWAY_WS_URL`；现有校验要求 HTTPS/WSS。默认值指向部署服务，不能未经确认
就拿真实账号做破坏性测试、部署或写线上数据。若要验证本分支修改，须使用获准且版本
可追踪的测试页面/网关，记录 Web 发布 SHA、资源 manifest/hash 与服务器版本；
“Mac Git 已更新”或“APK 刚构建”都不能证明远程页面是本分支版本。

## 4. 复用边界

| 内容 | 处理原则 |
| --- | --- |
| Gateway / Simulation、账号、规则、掉落和保存 | 继续连接同一套权威服务；不在手机另写权威战斗/背包/交易逻辑 |
| `client-core` / `ui-core` / 协议 | 共享状态、意图、预测和协议；审计 Android 的真实收发接线，缺失命令不能伪造 |
| `runtime` / `client-bevy` | 共享渲染、镜头、精灵/动画与 read model；验证移动 GPU、窗口生命周期与所需 feature |
| Crystal 地图、人物、装备、动画、音效 | 共用来源与版本，适配按需下载、缓存和纹理内存；不默认把全部资源塞进安装包 |
| Windows 地图/资源/网络生产逻辑 | 通用部分可小步提取至共享 native 模块；Windows 路径、Win32 输入、窗口接口不能照搬 |
| Web React/CSS | WebView 路线直接复用；原生 Bevy 不能直接运行这些组件 |
| 原版界面素材与业务语义 | 复用；触屏命中区、长按/拖拽、选怪、快捷栏、软键盘和安全区要适配 |
| Android 平台 | 安装签名、返回键、前后台、网络恢复、音频焦点、文件沙箱与设备回收需独立验证 |

不承诺未经验证的复用百分比、完成日期或原生性能优于 WebView。根据真机数据安排工作。

## 5. Mac 第一阶段：先跑可验证的 Web 壳体验

这是建议的首个有界交付，不表示 Android 原生路线已经放弃，也不要求同时完成两套 UI。
所有项目目前均待 Mac 实际执行；本次 Windows 发布没有代为启动 Mac 任务。

- [ ] **AND-MAC-00：基线。** 核验分支/提交、工作区、资源与允许使用的测试端点。
  检查 Node、项目 lockfile、JDK（现有 CI 为 21）、Android SDK / adb、设备或模拟器、
  空间；做原生检查时另外准备项目固定 Rust 1.95.0、Android target 和 NDK。
  不为了“能编译”批量升级 Bevy、Rust、Capacitor 或改变服务端协议。
- [ ] **AND-MAC-01：真实 APK。** 在 `apps/mir2-mobile` 先测试加载器，再 build、
  `cap sync android`、Gradle assembleDebug，核验输出文件、包名、SHA-256 与日志。
  安装前通过 `adb devices` 确认唯一目标，保留已有应用数据，不自动卸载/清空数据。
  Capacitor 包名是 `com.obelisklabs.mir2`；原生 scaffold 包名是 `com.mir2.web3`。
- [ ] **AND-MAC-02：玩家闭环。** 使用正常认证和玩家指令：登录/选角 → 比奇完整地图 →
  摇杆走跑与转向 → 选怪攻击 → 拾取 → 背包移动/使用/穿脱 → NPC 对话/基础任务 →
  切后台恢复 → 断网重连 → 退出重进检查状态。发现缺口逐项实现并回归，不用后台命令、
  假数据、自动授予物品或跳过按钮替代真实交互。
- [ ] **AND-MAC-03：设备与下一步报告。** 提交同一 APK 的截图/录屏、日志、问题清单和
  触控/网络/资源结论；给出原生 Android 最小联网场景的具体缺口和下一轮有界写集。

已确认测试端点、依赖及目标设备后，Web 壳的基本构建顺序是：

```bash
# 当前目录：mir2-web3/apps/mir2-mobile
# 先设置获准测试环境的 MIR2_MOBILE_GAME_URL / MIR2_GATEWAY_WS_URL。
npm ci
npm test
npm run build
npx cap sync android
(cd android && ./gradlew assembleDebug)
```

每一步失败就检查并处理，不能无条件继续宣称构建成功。设备脚本
`build-mobile-device.sh` 可作参考，但其中的本机 JDK/AVD 默认值、同步顺序和
“进程存在”检查需要核对；进程存活不等于登录、渲染或游戏闭环通过。

没有真机时可以继续构建、模拟器、离线单元测试和适配；将物理设备门槛明确标为未执行。
需要用户输入凭据、选择设备或授权端点时，单独列出最小缺项，不能自行寻找密钥/密码或
使用生产账号绕过。模拟器证据与真机证据分开记录。

## 6. 原生后续阶段的边界

Web 壳基线完成后，按证据推进独立原生小阶段，先真实 transport / 生命周期，再接登录、
地图/实体和触控闭环；不要把现有命令队列/FFI 当成已经实现的 WebSocket。缺少回执时
维持未知结果/保护状态，不通过超时重发非幂等购买、交易或物品指令来伪造成功。
重新审查恢复时的旧移动意图，不能断网后把积压走跑全部重放。

原生可以从 Windows 提取通用数据路径，但第一轮不大改 Windows 入口或一口气搬运全部
native UI。现有 Android `build-android.sh` 默认 check；package 入口也必须以真实工具
退出码、APK、安装和设备行为验证。Host 测试、Android target check、APK 打包、模拟器
与真机是五种不同证据，分别报告。

## 7. 文件分工与禁止事项

- Mac 首轮主要写集：`apps/mir2-mobile/**`、上述 Web 移动输入/控制组件及其测试、
  Android 专用 QA / handoff 文档；需要时再选取 `apps/game-client/platform-android/**`。
- `client-core`、`ui-core`、`client-bevy`、`runtime`、Web 主组件、全局任务队列/roadmap
  都是共享区。改前声明具体文件，确认没有其他 worker 同时写；按小步提交维护两端回归。
- Windows 继续负责原版对照、桌面显示与资源修复。Mac 不自动接管旧队列的 trade 后端
  leaf，不改 Windows 专用入口、账户存档、已运行服务或 F 盘资源目录。
- 第一阶段不扩张为后端权威/鉴权改造、schema 迁移、商业发布、线上部署或签名证书操作。
- 不暴露 `MoveTo`、`Stage5Command`、`qa.*`、裸 `PasskeyLogin { account_id }`，不启用
  demo 回退；Session 管个人状态，Zone 才是共享世界。手机不能伪装多个个人会话为多人。
- 保留现有失败与未完成项；不能删测试、跳过门槛、降低断言、清空数据或强推来获得绿色。

## 8. 交付和验收证据

每个里程碑记录：源码 SHA / 实际 Web 与 Gateway 版本、构建命令/退出码、APK 包名与
SHA-256、Android 版本/设备型号/WebView 版本、资源版本、是否真机、截图/录屏、
脱敏日志、已通过/失败/未测项。记录帧时间/内存/首屏与地图加载、持续运行和前后台恢复
的实际观察，不用桌面结果代替移动端测量。截图必须对应标注的构建与远程页面版本。

代码验证后分阶段 commit / push `codex/android-player-journey` 并提供独立 Draft PR，
不直接推回 Windows 分支、不合并任何 PR。正常交付包含可安装 APK 的可访问位置、
SHA-256、复现步骤、剩余缺口和下一步；不把 APK、大型缓存、账号资料或密钥放进源码库。

本文件只是交接上下文。Android 实现、构建、安装、联网与真机验收结果必须由 Mac 的
后续实际执行补齐，不能从本次 Windows 提交推送推导出来。
