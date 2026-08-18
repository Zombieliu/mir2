# qwen3-coder:30b-opt 任务规划（Worker Batches）

> 面向本地 `qwen3-coder:30b-opt`（~80 tok/s，16K ctx，非思考）的任务批次。
> 原则：**大批量、模式固定、单文件局部上下文、本地 gate 闭环**；架构/parity 语义
> 由 DeepSeek 主导、ChatGPT 兜底，本文档任务不越界。
> 状态列：⬜ 待办 / 🟨 进行中 / ✅ 完成 / ⛔ 不做
> 基线：仓库已对齐 origin/main `97887dcdb`（PR #239 三职业 50 级 parity）。

## P0 — 每次接手先做的绿灯维护（常驻批次）

> 2026-08-18 首轮扫描状态：`cargo fmt --check` ✅ / Web `npx tsc --noEmit` ✅ /
> `cargo test -p mir2-simulation` ✅（13 套件 ~1438 测试全绿，真退出码 0）/
> `cargo check` gateway/protocol/game-data ✅。
> `npm run test:frontend-logic` ⚠️ 38 个脚本中 1 个失败（见 P1-2a 红灯条目）。

| # | 任务 | 写文件域 | 验收 |
| --- | --- | --- | --- |
| P0-1 | 全量 gate 清零：`cargo fmt --check` + `cargo check --locked` 四包 + `cargo test --locked -p mir2-simulation -- --test-threads=1` + Web `npx tsc --noEmit` | 任意（按报错定位） | 全部绿灯 |
| P0-2 | 新提交回归补测：为最近合入功能（#239 skills parity、#238 quest icons、#237 world-director checkpoint、#236 供应路由恢复）补单元测试缺口 | `apps/simulation/src/runtime/**`、`packages/protocol`、`apps/web/lib` 各自的测试文件 | 相关 focused 测试通过 |

## P1 — 机械广度批（高吞吐甜区）

| # | 任务 | 写文件域 | 验收 |
| --- | --- | --- | --- |
| P1-1 | 协议五层接线模式扩展：按 Crystal 语义需求给 `ClientPacket`/`ServerPacket` 枚举补变体，穿透 gateway `server_packet_to_event` → `page.tsx` case → adapter | `packages/protocol/src/packets.rs`、`apps/gateway/src/web.rs`、`apps/web/app/page.tsx`、`apps/web/lib/stage5-window-adapters.ts` | 协议/网关测试 + `tsc --noEmit` + focused adapter 测试 |
| P1-2 | QA/资产脚本机械补丁：`apps/web/scripts/qa-*.mjs`、`capture-*.mjs`、certify-*.mjs 的新检查项与修复 | `apps/web/scripts/**`、`mir2-web3/scripts/**` | `node --check` + 目标脚本实跑 |
| **P1-2a** ✅ | **修复 `test-map-atlas-budget.mjs` 旧格式 manifest 崩溃**：本地 `public/generated/map-atlas/manifest.json` 为 schemaVersion 1（40 个 `atlases`、无 `pages` 数组），测试第 137 行直接 `manifest.pages.filter` 抛 TypeError。方向：读取后校验 `schemaVersion>=2 && Array.isArray(pages)`，否则与 128 行同风格 `context.skip()`（`--requireManifest` 变体除外）或改走 `mapAtlasManifestFitsBudget` 统一断言 | `apps/web/scripts/test-map-atlas-budget.mjs` | `npm run test:map-atlas-budget` 全绿 + `npm run test:frontend-logic` 无红灯 | **2026-08-18 由 qwen3-coder:30b-opt 完成**：模型 28.8s 产出 +9 行守卫（require 模式 `assert.fail`、非 require `context.skip`），其他测试零改动；`test:map-atlas-budget` 5 pass+1 skip exit 0；`test:frontend-logic` 全链 REAL_EXIT=0。观察项：`--requireManifest` flag 在 `node --test` 下疑似被吞（既有 CLI 行为，未处理）。 |
| P1-3 | manifest/数据生成脚本维护：`generate-crystal-runtime-manifests.mjs`、`import-crystal-localization.mjs`、localization bundle 新条目 | `packages/game-data/scripts/**`、`packages/tooling/scripts/**` | 生成 fixture 校验（game-data 测试） |
| P1-4 | 技能/数值表机械补齐：按 Crystal 常量/配置补三职业技能倍率、CD、MP 消耗、drop 表条目 | `packages/game-data` 数据文件 + 对应表加载测试 | game-data 测试 + focused sim 数值断言 |

## P2 — 模式化翻译/重构辅助（初稿给 coder，语义终审给 DeepSeek）

| # | 任务 | 写文件域 | 验收 |
| --- | --- | --- | --- |
| P2-1 | 怪物 AI handler 广度扩充：对照 Crystal 212 个 handler，把 C# 行为译为 Rust handler **初稿**（最大 gameplay 深度 gap） | `apps/simulation/src/runtime/monster_ai.rs`（高冲突，**遵守每轮单写者**） | DeepSeek 语义复审 + 全 sim 测试 + fmt |
| P2-2 | 经济/库存域拆域迁移脚本初稿：account JSON monolithic → inventory/mail/economy 独立域（设计已在任务队列） | `apps/simulation/src/runtime/save.rs` 周边的迁移/工具代码 | 迁移 fixture 全量通过 |
| P2-3 | 证据/文档机械更新：progress.md 轮次追加、AGENT-RUN-LOG、生成 evidence manifest 脚本 | `docs/progress.md`、`docs/AGENT-RUN-LOG.md`、scripts | diff 检查 + 文档格式校验 |

## ⛔ 明确不接（分配给 DeepSeek/ChatGPT/人工）

- 高冲突文件架构性改造（`runtime.rs`、`combat.rs` 等的设计级改动）
- auth/security、schema migration、production rollout、R2/部署链
- World Director 旧分支整批合入分类（`docs/WORLD-DIRECTOR-BRANCH-INTEGRATION.md`）
- 跨分支集成与冲突裁决（旧历史 174 提交 cherry-pick 到新主线）

## 执行纪律

1. 每批次开始前 `git fetch && git status`，基于最新 origin/main 起独立分支
2. 高冲突文件：开工前与协调者确认当前轮次无其他 worker 写入
3. 完成标准 = 本地 gate 全绿 + 自测证据落 `docs/generated/`（如适用）+ 更新本文件状态列
4. 不做未授权的大改：先列计划 → DeepSeek/协调者确认 → 再批量执行