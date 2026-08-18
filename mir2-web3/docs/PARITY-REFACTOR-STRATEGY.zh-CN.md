# 换语言全平台重构 · 对拍策略总纲（C# 原版 vs Rust 重构）

> 项目本质：**Legend of Mir 2 (Crystal) 由 C# 原版 → Rust + 全平台（Web/Bevy/原生）换语言重构**。
> 本总纲是全项目 parity 工作（逻辑层 + 画面层）的统一口径与分阶段路线，作为
> `AGENTS.md` / `CRYSTAL-1TO1-ROADMAP.md` 的对拍方法论补充。更新日期 2026-08-18。

## 1. 对拍的定义：为什么"原版连原版、重构连重构"是对的

换语言重构要证明的是：**同一操作/同一输入，C# 原版与 Rust 重构分别产出等价结果**。
所以两者天然是**两条独立链路**，不需要也不应该共用后端。

```
链路 A（参考实现）  Client.exe → Crystal Server.exe (:7000)     C# 服务器 + C# 渲染
链路 B（被重构）    Web(Bevy) / 原生 → Rust gateway → mir2-simulation   Rust 仿真 + Bevy/原生渲染
```

- 逻辑层等价 → packet / 后端状态 / 数值
- 画面层等价 → 同一场景在两渲染器的外观
- 跨端一致性 → 同一角色同一时刻两端表现（需"同后端 + 两渲染器"，架构上单独项）

## 2. 分层对拍总览

| 层 | 答的问题 | 工具/手段 | 现状 | 缺口 |
|---|---|---|---|---|
| **逻辑层** | 同输入 → 状态/包/数值是否一致 | `packet_trace`（`apps/gateway/src/bin/packet_trace.rs`）`--matrix` | R298/R300 达 **stable-diff 9/9 接受**；strict exact 因动态 AOI/volatile 仅诊断 | 覆盖玩法广度；同 fixture/同状态对齐 |
| **画面层** | 同场景 → 两渲染器外观 | CDP 抓 Web 帧 + 原版 Client 截屏 → pixel-diff / qwen2.5vl 描述 | Web+Bevy 现成 capture；VL 能语义审阅 | 两个客户端"同场景"对齐（原版需人工/录屏操作到同一地图/坐标/光照） |
| **跨端一致性** | 同账号/角色两端 | 需"同后端 + 双渲染器" | ❌ 架构未支持 | 需给 gateway 加"代理 Crystal"模式（大特性，见 §5） |

## 3. 逻辑层对拍（阶段一 · 当前重心）

**现成引擎**：`packet_trace --matrix`（local=本地 Rust 仿真，crystal=原版 Server via `MIR2_CRYSTAL_TCP_ADDR`）。
- 用法（见 `mir2-web3/docs/WINDOWS-CONTINUATION.md` / `PACKET-PARITY-ACCEPTANCE.md`）：
  `MIR2_CRYSTAL_TCP_ADDR=127.0.0.1:7000` + `packet_trace --matrix` → 产出 latest-matrix.json。
- **验收口径**：stable-diff 模式，`stableDiffDirtyCount=0`、`packetParityAccepted=true`。
  strict exact 仅作确定性 fixture 下的诊断。

**本阶段可做**：
- 把 `packet_trace` 覆盖从现有 9 个 flow 扩展到更多玩法（战斗/技能/NPC/掉落/地图切换）。
- 建立"可控状态对齐"的 Crystal fixture（关掉动态 AOI/volatile 干扰），让 strict exact 尽量接近。
- 每个新增 flow 存档到 `docs/generated/packet-traces/<round>-matrix/`。

## 4. 画面层对拍（阶段二）

**目标**：同一场景（同地图/坐标/光照），原版渲染器 vs Web+Bevy 渲染器的外观对比。

**手段**：
1. **像素级**：`apps/web/scripts/capture-*.mjs`（Web 侧 CDP）+ 原版 Client 截屏 → pixel diff / MAE。
2. **语义级**：抽帧 → `qwen2.5vl:7b-local`（`F:/mir2-tmp-vl/ask-vl.mjs`）逐帧描述、挑异常。
3. **铁律**：能被脚本量化的差异以脚本为准；VL 描述是线索，不是 gate。

**约束**：原版 Client 是图形窗口程序，**无法脚本化截图**，需人工操作/录屏到同一状态。
实践中让"人"把两个客户端分别推到同一地图/坐标/光照，再各自抓频。

## 5. 跨端一致性（阶段三 · 架构前置）

要让"同账号看两渲染器"，需**一个后端 + 双前端**。当前 Rust gateway 用的是
`InProcessWorldRuntime`（本地仿真），原版 Client 连原版 Server。要让二者真正同帧，
需评估给 gateway 增加"代理 Crystal Server"模式（把 Web 会话转发到原版 Server.exe），
这是**新的较大特性**，需先立项评审，不属于日常 parity 修补。

## 6. 视频模型（本地 VL）在本策略中的角色

`qwen2.5vl:7b-local` **不是对拍引擎**，是**语义级审阅助手**：
- 读本地 Web+Bevy 渲染帧，描述场景/角色/NPC/HUD/中文文案，挑视觉异常（已实测打通）。
- 但不能替代脚本像素对位；做不了精确偏移/MAE。
- 对原版 Client 画面同样可审（人工截屏后喂它）。
- **不做**：把 VL 输出当 CI gate、或当作对拍唯一依据。

## 7. 落地顺序（推荐）

1. **阶段一（逻辑层）** 优先——用现有 `packet_trace` 扩大玩法 flow 覆盖，巩固
   stable-diff 证据。这是重构等价性最硬、可自动化的部分。
2. **阶段二（画面层）** 用本地 VL + 现有 capture 先打通"Web+Bevy 帧审阅"（已做），
   再配合人工把原版 Client 推到同场景取证。
3. **阶段三（跨端一致性）** 待评估是否立项 gateway 代理 Crystal 模式。

## 8. 相关文档
- 逻辑层: `docs/PACKET-PARITY-ACCEPTANCE.md`、`docs/WINDOWS-CONTINUATION.md`
- 对拍总纲: `docs/CRYSTAL-1TO1-ROADMAP.md`、`docs/PARITY-TRUTH-AUDIT.md`
- 画面/VL: `docs/VIDEO-REVIEW-WORKFLOW.zh-CN.md`、`docs/VIDEO-REVIEW-RUNBOOK.zh-CN.md`
- 模型分工: `docs/MODEL-DIVISION.zh-CN.md`
