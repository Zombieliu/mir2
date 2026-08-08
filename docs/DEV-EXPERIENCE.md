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

项目根在 `~/obelisk/numeron`。**注意:这个目录不是标准 git 布局**(无 .git、无 Crystal 子模块),但可完整编译运行。

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
