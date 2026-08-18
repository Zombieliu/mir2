# 视频/画面纠错工作流（Local VL + Script 双轨）

> 面向 mir2-web3 的"玩游戏→发现画面问题"场景。核心原则：
> **像素级对位交给脚本管线，语义级审查交给视觉模型**——两者互补，不要互相替代。
> 本地视觉模型：`qwen2.5vl:7b-local`（Ollama，9.5GB）。更新日期 2026-08-18。

## 0. 一图看懂：怎么分工

```
游戏录像/录屏(MP4)
        │
        ├─(脚本跑对拍)────────────────────────────┐
        │   CDP/capture 管线                       │  像素级：位置/偏移/MAE/rollback
        │   (capture-crystal-parity /              │  客观可复现，CI 可断言
        │    capture-web-movement-jitter /         │
        │    pixel-diff / MAE)                     │   ← 权威依据，出结论以它为准
        │                                          │
        └─(ffmpeg 抽帧 → qwen2.5vl 逐帧)──────────┘
            ask-vl.mjs  一句话｜具体到点            语义级：看到了什么/哪不对劲
            描述画面 + 挑异常                       主观/模糊，用于定位线索、写报告
```

## 1. 什么时候用哪条轨

| 问题 | 用脚本管线 | 用本地 VL |
| --- | --- | --- |
| "玩家位置差了几个像素 / 卡顿 / 回滚" | ✅ 权威 | 辅助描述 |
| "这一帧画面和 Crystal 原版差在哪" | ✅ pixel diff / MAE | 辅助定性 |
| "这段录像里整体发生了什么 / 有没有明显异常" | — | ✅ 主力 |
| "UI 上显示的数值、文案、弹窗内容" | 需 OCR | ✅ 直接读图（能读中文） |
| "哪个 NPC/怪物/角色出现在画面哪个角落" | 边界盒 | ✅ 自然语言定位 |

> 铁律：**任何能被脚本量化的差异，都以脚本结果为准**。VL 的描述不能替代
> capture-*.mjs 的数值断言作为 CI gate。

## 2. 本地视觉模型用法

### 2.1 单张截图
```bash
node F:/mir2-tmp-vl/ask-vl.mjs qwen2.5vl:7b-local 截图.png
```

### 2.2 视频抽帧后逐帧（模拟"看视频"）
```bash
# 抽帧（fps=1，每 1 秒一帧）
ffmpeg -i gameplay.mp4 -vf fps=1 extracted_%02d.png
# 逐帧喂模型（一次可多张）
node F:/mir2-tmp-vl/ask-vl.mjs qwen2.5vl:7b-local extracted_01.png extracted_02.png ...
```

### 2.3 自定义审查提示词
```bash
VL_PROMPT="只报告这一帧里与角色/怪物/UI相关的视觉异常，没有就回复'无异常'" node F:/mir2-tmp-vl/ask-vl.mjs qwen2.5vl:7b-local 帧.png
```

## 3. 实时链路（工具清单）

| 工具 | 位置 | 用途 |
| --- | --- | --- |
| `ask-vl.mjs` | `F:\mir2-tmp-vl` | 图片→本地VL 描述的通用脚本 |
| ffmpeg | WinGet 安装（`Gyan.FFmpeg`）| 视频合成/抽帧 |
| 本地 VL 模型 | `qwen2.5vl:7b-local`（Ollama）| 看画面 |
| CDP 对拍脚本 | `apps/web/scripts/capture-*.mjs` | 像素级权威对拍 |
| 证据目录 | `docs/generated/player-qa/*` | 截图/state.json 归档 |

## 4. ⚠️ 已知约束

- **显存竞争**：`qwen2.5vl:7b-local` 与 `qwen3-coder:30b-opt` 抢同一块 16GB 卡
  （`OLLAMA_MAX_LOADED_MODELS=1`），**同一时刻只能驻留一个**。切换≈15s。
  → 看视频会挤出 coder，干完切回编码记得确认 coder 重新加载。
- **只能语义级**：16GB 只跑得动 7B 级视觉模型，它做不了像素级精确对位。
  更强的 32B 视觉模型需更大显存。
- **非流式视频**：目前是"抽帧→逐帧问"，不是真正的时序视频模型。
  若要连续场景理解，可用多帧 batch（见 2.2）。
- **LLM 语义 ≠ 证据**：VL 输出的"异常"是线索，落地 bug 修复前必须用脚本管线
  验证，并把截图证据归档到 `docs/generated/player-qa/`。

## 5. Demo 已验证（2026-08-18）

- 输入：`combat-survival-default-selfcamera-rust7111-bichonstarter-20260708/frames/`
  的 8 帧游戏流程截图（登录→加载→村庄→战斗）。
- 合成 4s 视频 → 抽 4 帧 → **逐帧审核成功**（单帧 3~4s，首载 18s）。
- 模型正确识别：角色/Royal Guard/Archer、HP 18/18、Bichon 地图、HUD、
  "Welcome to Mir 2" 教程弹窗，**并主动指出"异常"**（如 NPC 站位突兀）。

## 6. 本工作流与现有团队分工的关系

- 无冲突：这是给 **ChatGPT**（云前端视觉审查）增加了一个**本地离线后备/加速**选项，
  不改变"像素级对拍归脚本、语义审查可多选"的既有设计。
- ChatGPT 仍负责需要 frontier 判断或多模态精细分析的画面结论；
  本地 VL 适合快速、批量、离线、隐私场景（如反复调参时的快速画面确认）。