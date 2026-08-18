# 「原版 + 本地工程 + 视频模型」对拍运行手册

> 目标：把 **原版 Crystal 客户端** 与 **本地 Web 工程** 都真实跑起来，
> 用「抽帧 + 本地视觉模型(qwen2.5vl)语义审查」对比两条渲染通道的画面。
> 更新日期 2026-08-18。此链路已实测打通后端。

> **⚠️ 架构澄清（2026-08-18）**：本地 Web **并不连接原版 Crystal Server**。本手册
> 早期版本误写"两条渲染通道共用后端"，是错误的。实际是**两套相互独立的后端**：
>
> ```
> 原版 Client.exe ──TCP──► 原版 Crystal Server.exe (:7000)   ← C# 服务器 + 原版渲染
> 本地 Web/Bevy   ──WS──► 本地 gateway ─► 本地 Rust mir2-simulation  ← 自研仿真 + Bevy
>                         (InProcessWorldRuntime，进程内)
> ```
>
> 二者是**换语言重构的两条独立实现链路**，不共享数据。
> `MIR2_CRYSTAL_TCP_ADDR` 仅在 `packet_trace` 工具中用于「原版 vs Rust 仿真」的
> packet 级对比，并不把 Web 会话代理到原版 Server。

## 0. 一图看懂：运行时拓扑（正确版）

```
┌─ 链路 A：C# 原版 ─────────────────────────────┐
│ Client.exe → Crystal Server.exe (:7000)       │  参考实现（原始 C# 服务器/渲染）
└───────────────────────────────────────────────┘
┌─ 链路 B：Rust 重构 ───────────────────────────┐
│ 本地 Web(Bevy,:3002) → gateway(7010/7310)     │  被重构实现（Rust 仿真/渲染）
│   → InProcessWorldRuntime(mir2-simulation)    │
└───────────────────────────────────────────────┘
         ↓ 对比目标（重构项目的核心）
   同一操作·同输入 → 原版输出 vs Rust 输出是否一致
   - 逻辑层: packet_trace 逐包/状态对比
   - 画面层: 同场景各自截图 → pixel-diff / qwen2.5vl 语义审查
```

- **为什么分两条链路**：这是**换语言全平台重构**。对拍 = 证明 Rust 实现与 C# 原版
  在同样的输入下行为等价（1:1 parity），而非让两个客户端连同一后端。
- 逻辑层对拍已有现成工具：`apps/gateway/src/bin/packet_trace.rs`（R298/R300 达
  stable-diff 9/9 接受）。画面层对拍需"两个客户端到同一状态"，见策略总纲。

## 1. 启动顺序（已在 E:\mir2 验证）

### 1.1 原版 Crystal Server（监听 7000）
```powershell
& 'E:\mir2\Crystal\Build\Server\Debug\Server.exe'
# 端口在 E:\mir2\Crystal\Build\Server\Debug\Setup.ini 的 Port= 字段
# 验证：Get-NetTCPConnection -State Listen -LocalPort 7000
# 日志：E:\mir2\Crystal\Build\Server\Debug\Logs\Server\Server (日期).log → "Network Started"
```

### 1.2 本地 gateway（给 Web 提供数据）
```powershell
cd E:\mir2\mir2-web3
$env:MIR2_GATEWAY_TCP_ADDR='127.0.0.1:7310'   # 避开 Crystal 的 7000
$env:MIR2_GATEWAY_WEB_ADDR='127.0.0.1:7010'   # Web 从这里拿 HTTP/WS
$env:MIR2_ALLOW_DEV_IDENTITY_SECRETS='1'       # 本地开发身份密钥
target\debug\mir2-gateway.exe
# 验证：Invoke-RestMethod http://127.0.0.1:7010/health → {ok:true}
# 若报 MIR2_IDENTITY_SESSION_SECRET 未设置，加 $env:MIR2_ALLOW_DEV_IDENTITY_SECRETS='1'
```

### 1.3 本地 Web 前端（连 gateway 7010 的 WS）
```powershell
cd E:\mir2\mir2-web3\apps\web
$env:NEXT_PUBLIC_MIR2_GATEWAY_WS_URL='ws://127.0.0.1:7010/ws'
$env:PORT='3002'
npx next dev -p 3002          # 快速验证；完整 dev 用 npm run dev
# 访问 http://127.0.0.1:3002
```

> **提示**：`next dev` 直起会跳过 bevy WASM 运行时（画面用 DOM 降级渲染，也能用）。
> 要原生 Bevy 渲染，须先 `node scripts/build-bevy-runtime.mjs release` 再重开 next。

### 1.4 本地视觉模型（图→描述）
```powershell
# 若已删除，从 Modelfile 重建：
& 'C:\Users\Administrator\AppData\Local\Programs\Ollama\ollama.exe' create qwen2.5vl:7b-local -f 'F:\mir2-tmp-vl\Modelfile.qwen2.5vl'
# 单图：
node F:/mir2-tmp-vl/ask-vl.mjs qwen2.5vl:7b-local 某图.png
# 多图（视频抽帧批量）：
node F:/mir2-tmp-vl/ask-vl.mjs qwen2.5vl:7b-local 帧1.png 帧2.png ...
```

## 2. 抓 Web 画面（CDP 自动化）
```powershell
cd E:\mir2\mir2-web3\apps\web
$env:NEXT_PUBLIC_MIR2_GATEWAY_WS_URL='ws://127.0.0.1:7010/ws'
node scripts/capture-crystal-parity.mjs --base-url http://127.0.0.1:3002 `
  --out-dir F:\mir2-tmp-vl\web-cap --map 0 --x 287 --y 618
# 产出：png 截图 + state.json（screen/player/HUD/consoleErrors）
```

## 3. 抓原版 Client 画面（有限人力）
- 原版 `Client.exe` 是图形窗口程序，**无脚本化截图通道**。
- 做法：人工登录进游戏 → 截屏/录屏(ffmpeg gdigrab 或系统截图) → 抽帧喂 VL。

```bash
ffmpeg -f gdigrab -framerate 2 -i desktop -t 5 original_clip.mp4   # 抓桌面
ffmpeg -i original_clip.mp4 -vf fps=1 orig_%02d.png                 # 抽帧
node F:/mir2-tmp-vl/ask-vl.mjs qwen2.5vl:7b-local orig_01.png ...
```

## 4. 推荐工作流

| 步骤 | 手段 | 目的 |
| --- | --- | --- |
| 像素级对位 | `capture-crystal-parity.mjs` + state.json 断言 / pixel diff | 客观差异（权威）|
| 语义级审阅 | 抽帧 → `ask-vl.mjs` → qwen2.5vl 描述 | 看见了什么、哪不对劲、读中文文案 |
| 汇总报告 | 收集 VL 描述 + state.json → 对照原版 | 定位需人工确认的 visual gap |

> **铁律**：VL 的"异常"是线索；落地修复前必须用脚本对拍验证，并把证据归档 `docs/generated/player-qa/`。

## 5. 已知约束
- **显存竞争**：`qwen2.5vl:7b-local` 与 `qwen3-coder:30b-opt` 抢同一块 16GB 卡
  （`OLLAMA_MAX_LOADED_MODELS=1`），同时只能驻留一个；切换≈15s。
- 原版 Client 无法自动截图 → 该通道需配合人工。
- `next dev` 直起缺 bevy runtime（DOM 渲染可用）；原生渲染需先 build runtime。
- VL 只能语义级，做不了像素精确对位；强度受 16GB 显存限制（7B）。

## 6. 本次已实测打通（2026-08-18）
- 原版 `Server.exe` 7000 ✅（463 图/Envir/Network Started）
- gateway 7010(Web)/7310(TCP) ✅ health ok
- web 3002 ✅ Next ready
- **CDP 抓 Web 游戏帧 → qwen2.5vl 描述 ✅**（识别 QA0429Hero、NPC、HUD、chat、小地图，判"无异常"）
