# 模型分工说明（Model Division）

> 本文档定义 mir2-web3 开发中四份推理资源的定位、任务边界与切换流程。
> 更新日期：2026-08-18。《AGENTS.md》的 Model Policy 与本文档不冲突时照旧；
> 冲突时以 AGENTS.md 为准。

## 0. 资源清单（校准数据为 2026-08-18 本机实测）

| 资源 | 通道 | 生成速度(实测) | 上下文(实际) | 定位 |
| --- | --- | --- | --- | --- |
| `qwen3-coder:30b-opt` | 本地 Ollama `127.0.0.1:11434` | **~80 tok/s**，TTFT 0.7s | 16K（显存约束） | 编码特化（MoE 30.5B/激活3B）、**非思考**、工具调用✅ |
| `qwen3.8:27b-opt` | 本地 Ollama 同上 | ~14.4 tok/s | 16K | 通用 dense 27B（qwen35 混合架构）、思考模式（DSH 配置已禁用） |
| DeepSeek（订阅） | 云端（DSH 默认 agent 模型 `deepseek-v4-flash`） | 快 | 大 | 通用 + 推理，长程 agent 主力 |
| ChatGPT（订阅） | 云端 | 快 | 大 | frontier 推理 + **多模态（看图）** |

本地两台模型共享一块 RTX 5070 Ti 16GB 显存，`OLLAMA_MAX_LOADED_MODELS=1`：
**同一时刻只能驻留一个本地模型**，切换会触发卸载/重载（约 10~20s）。

## 1. 任务类型 → 模型映射

| 任务类型 | 首选 | 说明 / 项目内例子 |
| --- | --- | --- |
| 高频机械编码（批量、模式固定、局部上下文） | **qwen3-coder:30b-opt** | 修 `fmt`/`tsc` 红绿、补单元测试、协议五层接线、manifest/QA 脚本补丁 |
| 深度推理/架构/parity 语义 | **DeepSeek** | Crystal 1:1 语义对齐（`file:line` 引用）、Zone MVP 设计、跨 crate 演进、长程任务规划 |
| 尖峰推理/生产安全/多模态 | **ChatGPT** | auth/security、schema 迁移、production rollout、破坏性清理（AGENTS.md 要求 frontier 主导）；CDP 截图/UI 渲染看板 |
| 通用文本/文档批处理 | **qwen3.8:27b-opt**（弱优先级） | 文档翻译、progress/报告润色、localization 文本整理 |

### 1.1 qwen3-coder:30b-opt 适合

- 把 "Crystal C# 逻辑 → Rust 等价物" 的**机械翻译初稿**（细粒度语义核对交给 DeepSeek 审）
- 批量测试补丁、类型修复（`cargo fmt --check` / `npx tsc --noEmit` / 单文件测试）
- 协议枚举/网关 bridge/`page.tsx` case/adapter 的**模式化接线**
- `.mjs` 脚本（QA、manifest 生成、certify、capture）的新增与修复
- 代码评审草稿（给一段 diff 快速挑错）

### 1.2 qwen3.8:27b-opt 定位

- 仅用于**非交互批量文本**：中文文档翻译/润色、数据描述生成
- 编码任务不推荐（速度只有 coder 的 1/5.5；思考模式已被禁用，通用强项被削）
- 若 coder 模型正在驻留，不为其专门切换

### 1.3 DeepSeek（默认主力）

- DSH 会话默认 `agent-default-model: opencode-go/deepseek-v4-flash`，保持现状
- 架构设计、跨文件一致性、Crystal parity 语义、Zone/共享状态机、任务队列拆解

### 1.4 ChatGPT

- **多模态验证**（本项目不可替代）：CDP 截图、地图/HUD 渲染效果、Bevy 画面问题
- 生产级变更主导：auth/security、schema 迁移、production rollout、destructive cleanup
- 跨分支集成与冲突裁决（如旧历史 cherry-pick 到新主线）

### 1.5 红线（任何本地模型都不参与）

- R2 资产发布链 / Vercel 部署 / Cloudflare 代理（凭据与流程在 Codex 侧）
- 生产账号/支付/加密相关改动
- 高冲突文件（如 `apps/simulation/src/runtime.rs`）**每轮仅一个代码 worker 编辑**
- World Director 旧分支整批合入决策（见 `docs/WORLD-DIRECTOR-BRANCH-INTEGRATION.md`）

## 2. 切换流程

1. **DSH GUI**：会话模型选择器切到 `qwen3-coder:30b-opt`（设置里已注册，见 `C:\Users\Administrator\.dsh\settings.yaml` 的 `llm-deepseek.models`）。
2. 若本地 API 未启动：`ollama serve`（环境变量已持久化：`OLLAMA_MODELS=F:\ollama\models`、`KEEP_ALIVE=-1`、`MAX_LOADED_MODELS=1`、`KV_CACHE_TYPE=q8_0`）。
3. 切换本地模型注意单驻留：调 `http://127.0.0.1:11434/api/ps` 查看当前驻留，避免冷载等待。

## 3. 模型输出验收原则

- **任何模型产出的代码必须过本地 gate 才能算完成**：
  `cargo fmt --check` → `cargo check --locked -p <crate>` → 相关测试 → `npx tsc --noEmit`
- 大模型初稿 ≠ 完成；架构/parity 语义结论必须有 Crystal 源引用（`file:line`）
- 文档/任务队列更新：backend 改动→`BACKEND-1TO1-PROGRESS.md` 等；frontend 改动→玩家 QA / frontend gaps 文档