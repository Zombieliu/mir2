# 链上智能矿场实施路线图（On-Chain Smart Mine — Implementation Roadmap, M0→M8）

> 状态：v0.1 草案（2026-06-09）。本路线图把 [`ONCHAIN-SMART-MINE-DESIGN.md`](./ONCHAIN-SMART-MINE-DESIGN.md)
> 的设计（Dubhe schema §3.1、`mine_batch` §3.2、集成时序 §4、经济 §5、并发/防作弊 §6、分阶段 §8）
> 拆成**可执行、可验收的里程碑**。它是实现期的"权威执行文档"——每个里程碑必须满足其 **Exit 出口判据**
> 才能进入下一个，并在结束时**停下汇报**。
>
> **设计 vs 路线图分工**：DESIGN 回答"做成什么样、为什么"；本文回答"按什么顺序做、每步交付什么、怎么算完成"。
> 二者冲突时，**经济/锁定决策以 DESIGN §0.9 为准**，**工程顺序与出口判据以本文为准**。
>
> 读者：链上（Move/Dubhe）、Relayer/索引、模拟（Rust/`bevy_ecs`）、网关、前端（Next/Bevy）、经济/运维。

---

## 0. 阅读指引 & 全局约束（任何里程碑都不许破）

### 0.1 工作方式

- **功能分支**：`feat/onchain-mine`，所有改动在分支上；review-ready 可开 PR，**未经确认不合并**。
- **onchain 栈独立**：全部放 `mir2-web3/onchain/`，**不混入 Rust workspace（`mir2-web3/Cargo.toml` 显式 members，不含 onchain）/ 任何 pnpm root（仓库无 `pnpm-workspace.yaml`，onchain 为独立 TS 包）**。
- **一次一个里程碑**：达成 Exit → 跑全门禁 → 每个逻辑块一个 commit → **停下汇报**（做了什么 / 交付物：testnet packageId·tx digest·事件内容·测试结果 / 下一里程碑计划）。
- **必须停下来问**：进 **M5（经济/治理）**前、进 **M7（上服务器）/ M8（主网）**前、以及**任何动到架构或需求有歧义**处。

### 0.2 五条硬约束（贯穿 M1→M8，cross-cutting，见 §10）

1. **服务端始终权威**：链确认前只播**乐观特效**；真正发矿/给金币只在 Relayer 注入"链确认事件"后由 Sim 落库。
2. **不破坏 P0 服务端挖矿，新老并存**：`config.mine_zones` / `runtime/mining.rs`（`MineSpot`/`try_mine`/`give_mine_payout`/`mine_stage`）/ `ServerPacket::MineNodeState` **已实现且在跑**——链上层是**叠加**，不是替换。
3. **三处幂等防双发**：①链上 `miner_nonce` 严格递增；②Relayer 用 `(tx_digest, event_seq)` 去重；③Sim 入站命令带幂等键。
4. **经济护栏**：批量结算（非逐挥）、`sui::random` 链上随机、节点可再生 + **每 epoch 排放封顶**、费入 `treasury`、**先做纯付费货币**（无外部回兑）。
5. **全程 Sui testnet**；不碰生产 gateway；不上主网——除非走到 M8 且明确 go。**私钥/助记词/operator token 只进本地 env / gitignore，绝不提交。**

### 0.3 现状基线（M0 起点，已核验）

| 维度 | 现状 | 证据 |
|---|---|---|
| **P0 服务端挖矿** | ✅ **已完成** | `apps/simulation/src/runtime/mining.rs`：`MineSpot{mine_set_index,stones_left,last_regen_tick}`、`try_mine`、`give_mine_payout`、`mine_stage`(3 档)、`roll`、`MineNodeState` 广播 |
| 渲染包 | ✅ 已有 | `packages/protocol/src/packets.rs`：`ServerPacket::MineNodeState{location,stage}`（encode/decode 齐备） |
| 矿区数据结构 | ✅ 已有 | `apps/simulation/src/config.rs`：`MineZoneRecord{map_file_name,mine_set,...}`、`mine_zones` |
| 链→Sim 注入口 | ✅ 可扩展 | `apps/simulation/src/world_runtime.rs`：`WorldCommand` / `WorldCommandKind`（含 `Stage5Command` 先例 + `WorldCommandOutcome`/`WorldCommandExecution`） |
| Sim→外部事件总线 | ✅ 可复用 | `apps/gateway/src/events.rs`：`GatewayGameplayEvent`、`GameplayEventSink` trait、`RedpandaGameplayEventSink`、topic `gameplay.command.executed` |
| 钱包↔账号 | ✅ 同构 | `apps/gateway/src/auth.rs`：`account_id == "sui:0x.."` |
| 客户端收包 | ✅ 入口在 | `apps/web/app/page.tsx` packet `switch`（`case "MineNodeState"` 可挂） |
| 链上合约 / Indexer / Relayer / 钱包前端 | ❌ **本路线图新建** | `mir2-web3/onchain/**` |

> **结论**：DESIGN §8 的 **P0 已落地**。本路线图从"上链"开始（≈ DESIGN P1+），P0 是必须保活的并存基线。

### 0.4 工具链版本（M0 锁定）

| 工具 | 版本 | 用途 |
|---|---|---|
| Sui CLI | 1.68.1（homebrew，本机） / CI 用 pinned 预编译 testnet 二进制 | Move 编译/测试/部署、钱包、faucet |
| Dubhe SDK | `@0xobelisk/sui-common` `sui-client` `graphql-client` `ecs` `=1.2.0-pre.96`；`@0xobelisk/sui-indexer` `=1.2.0-pre.51`；`@0xobelisk/sui-cli` `=1.2.0-pre.96`（提供 `dubhe` 命令：schemagen/publish） | config/schemagen/tx/parseState/indexer |
| Node / pnpm | 22.21.1 / 9.15.4 | onchain TS 包 |
| Rust / cargo | 1.87.0（本机）；CI 候选门禁用 1.89.0 + rustfmt | Sim/网关（保活 + M3 入站） |

> 版本为 `*-pre.*` **预发布**，M0 在 `onchain/package.json` 用 `=` **精确锁定**，避免漂移。

---

## 1. 七条工作流（Workflows / 并行车道）

链上矿场跨七条车道。里程碑是"垂直切片"（跨车道交付一个可验收能力）；车道是"水平关注点"（贯穿多个里程碑）。

| # | 工作流 | 责任面 | 代码落点 | 首次出现 |
|---|---|---|---|---|
| **WF-1** | **Contracts**（Move/Dubhe） | schema、`mine_batch`/`redeem`/`admin`(/`refine`) 系统、`sui::random`、nonce、treasury、排放封顶、Move 单测、部署 | `onchain/contracts/mir2_mine/**`、`onchain/dubhe.config.ts` | M1 |
| **WF-2** | **Indexer**（Dubhe Indexer + GraphQL） | 订阅链上事件流 → GraphQL/订阅端点 | `onchain/indexer/**`、`dubhe.config.json` | M2 |
| **WF-3** | **Relayer/Bridge**（新微服务） | GraphQL ws 订阅 → `(tx_digest,event_seq)` 去重 → 归一化 → 调 gateway/admin-api 注入幂等命令 | `onchain/relayer/**` | M2 |
| **WF-4** | **Simulation**（Rust 权威） | 新 `WorldCommand` 变体（`GrantOnchainOre`/`CreditGoldFromOre`）、幂等键、矿石入背包、给金币、对账乐观结果、广播渲染；**与 P0 `mining.rs` 并存** | `apps/simulation/src/world_runtime.rs`、`runtime/mining.rs`、`config.rs` | M3 |
| **WF-5** | **Gateway**（边缘） | 出站审计/风控事件（`events.rs` 复用）、入站注入端点、`sui:0x` 身份绑定、命令校验 | `apps/gateway/src/{events.rs,auth.rs,web.rs}`、`apps/admin-api/**` | M3 |
| **WF-6** | **Client**（Next/Bevy） | Sui 钱包连接、攒 N 挥 → `mine_batch`（sponsored）、乐观 VFX、链确认对账、按链上 `stones_left` 渲染分档、`redeem` UI | `apps/web/app/page.tsx`、`apps/web/lib/**`、`apps/game-client/runtime/**` | M4 |
| **WF-7** | **Economy/Ops**（治理/可观测） | `admin_system` 参数（费率/封顶/限速/兑率）、反女巫、treasury 对账、监控、灰度、安全审计 | `onchain/contracts`（admin）、`onchain/ops/**`、dashboards | M5 |

并行点：**M2（WF-2/3）∥ M3（WF-4/5）**——Relayer/Indexer 与 Sim/Gateway 入站可各自对 mock 开发，M4 合龙。

---

## 2. 里程碑总览 & 关键路径

```
M0 Foundation ─▶ M1 Contract-MVP ─▶ ┌─ M2 Off-chain Bridge (WF-2/3) ─┐
  (本次)            (testnet)        │                                 ├─▶ M4 E2E Vertical Slice
                                     └─ M3 Sim/Gateway Inbound (WF-4/5)┘        (testnet, 端到端)
                                                                                    │
                                                                                    ▼
                          M8 ◀── M7 ◀── M6 Hardening ◀── M5 Economy/Governance ◀────┘
                       (主网)  (上服务器) (分片/sponsor/   (★停下问：费率/封顶/兑率/
                       ★go    ★go/no-go   安全/refine/压测)   限速/合规 — DESIGN §5/§9)
```

**关键路径**：`M0 → M1 → (M2 ∥ M3) → M4 → M5 → M6 → M7 → M8`。唯一并行分叉是 M2∥M3。
**强制停点**：M0 末、M4 末（汇报 E2E）、**M5 前（经济拍板）**、**M7 前 / M8 前（go/no-go）**。

| 里程碑 | 一句话 | 碰链 | 工作流 | DESIGN 对应 |
|---|---|---|---|---|
| **M0** | 工具链 + `onchain/` 工作区 + CI + testnet 钱包 | 否（仅充值） | 全 WF 的地基 | — |
| **M1** | 合约 MVP 上 testnet（schema+`mine_batch`+`redeem`+`admin`，Move 单测，TS 烟测） | ✅ testnet | WF-1(+WF-6 烟测) | P1 §3 |
| **M2** | Indexer + Relayer：订阅 `mine_settled`/`ore_redeemed` → 归一化注入（去重） | ✅ | WF-2, WF-3 | P2 §4 |
| **M3** | Sim/Gateway 入站：`WorldCommand` 新变体 + 幂等键 + 矿石入包/给金币（mock 驱动） | 否（mock） | WF-4, WF-5 | §4 |
| **M4** | **端到端垂直切片**：客户端 `mine_batch` → 链 → Indexer → Relayer → Sim → 背包 + 渲染对账 | ✅ | 全部合龙 | §4 全图 |
| **M5** | **经济/治理（★停下拍板）**：费率/批大小/排放封顶/兑率/限速/合规做成链上治理参数 | ✅ | WF-7 | §5 §6 §9 |
| **M6** | 加固：热点分片、sponsored+批量、对账健壮性、`refine_system`(可选)、安全、压测 | ✅ | 全 WF | P4 §6 §5.0+ |
| **M7** | **上（staging）服务器（★go/no-go）**：接真实 gateway/sim 部署、运维监控、灰度 | ✅ | WF-5/7 | P4 |
| **M8** | **主网（★go）**：审计、主网部署、风控接 Admin | ✅ mainnet | WF-1/7 | P5 |

---

## 3. M0 — Foundation（工具链 + 工作区 + CI + testnet 钱包）★本次执行

**Goal**：把 onchain 开发地基铺好，使 M1 可以"开箱即写合约"；CI 绿；testnet 钱包就绪。**不写业务合约**。

**Tasks**
1. 脚手架 `mir2-web3/onchain/`（隔离于 Rust/pnpm root）：
   - `contracts/mir2_mine/`（Sui Move 包骨架，含 `Move.toml` + 占位 `sources/*.move` + 1 个占位测试；M1 用 schemagen 产物替换/扩充）。
   - `package.json`（独立 TS 包，精确锁定 Dubhe 依赖 + `dubhe` CLI；脚本 `build`/`typecheck`/`move:test`/`schemagen`/`wallet:*`）。
   - `tsconfig.json`、`dubhe.config.ts` 占位（M1 照 §3.1 填实）。
   - `.gitignore`（Move `build/`、`.env*`、`*.key`/keystore、`node_modules`）、`.env.example`（仅占位，无密钥）、`README.md`（结构 + 命令 + 安全须知）。
2. 工具链：确认 Sui CLI 可用；安装并验证 `dubhe`（`@0xobelisk/sui-cli`）命令可跑；记录版本（§0.4）。
3. CI：`.github/workflows/mir2-ci.yml` 加 **path-gated** `onchain` job（仅 `mir2-web3/onchain/**` 触发）：装 Node + pinned Sui 二进制 → `sui move test` + TS `typecheck`/`build`。
4. testnet 钱包：生成**专用**密钥（不动现有地址）、`switch --env testnet`、faucet 充值；**私钥留在本地 keystore/env，仓库零密钥**；导出地址（仅地址）。

**File targets**（新建）
- `onchain/contracts/mir2_mine/{Move.toml,sources/mir2_mine.move,tests/*.move}`
- `onchain/{package.json,tsconfig.json,dubhe.config.ts,.gitignore,.env.example,README.md}`
- `.github/workflows/mir2-ci.yml`（新增 `onchain` job + paths-filter 增加 `onchain` 过滤）

**Deliverables**：可 build 的工作区骨架；CLI 版本清单；testnet 资助地址；CI 绿。

**Exit 出口判据**
- [ ] `mir2-web3/onchain/` 存在且**隔离**（`cargo metadata` 不含它；无 pnpm root 牵连）。
- [ ] `cd onchain/contracts/mir2_mine && sui move build && sui move test` **绿**。
- [ ] `cd onchain && pnpm install && pnpm typecheck && pnpm build` **绿**；`pnpm dubhe --help`（或等价）可跑。
- [ ] CI `onchain` job 绿；既有 `rust-workspace`/`web-resource-gate`/`local-candidate-gate` 不受影响（M0 不碰 Rust/web）。
- [ ] testnet 专用地址已充值（faucet ≥ 1 SUI）；`sui client active-env == testnet`；**`git status` / `git check-ignore` 证明无密钥入库**，`.env.example` 仅占位。
- [ ] 本路线图 + 工作区骨架已提交（逻辑分块 commit）。

**Dependencies**：无（地基）。
**Risks**：①Dubhe 预发布版本/peer-dep 漂移 → 用 `=` 精确锁定、M0 仅装、M1 验证 schemagen。②CI 装 Sui 二进制慢/版本漂移 → pin release tag + 缓存。③误改全局 `sui` active env → 记录原值（`localnet`）、可一键回退。

---

## 4. M1 — Contract MVP on testnet（WF-1）

> **Status（2026-06-09）：✅ DONE.** packageId `0xe6c3602e…40dbe5`、shared Schema `0x77138cee…cc698` 上 testnet（`onchain/deployments/testnet.json`）；**Move 单测 13/13**；`mine_batch` 结算/给矿/`mine_settled` 事件、nonce 重放守卫、枯竭 `mine_depleted` 均**链上验证通过**；TS 烟测 `scripts/smoke-mine.ts` 已写（typecheck 绿，未由 agent 运行——需用户把私钥放进 `.env`）。**工具链发现**：必须把 Sui CLI 升到 **1.73.0**（testnet protocol 126），1.68.1 会 `PublishUpgradeMissingDependency`；Move.toml `Sui` override 同步到 `testnet-v1.73.0`，CI 二进制同步。

**Goal**：把 DESIGN §3 的合约写实、Move 单测覆盖核心不变量、部署 testnet、TS 跑通一笔 `mine_batch` + 一笔 `redeem`。

**Tasks**
1. `dubhe.config.ts` 照 **DESIGN §3.1**：`data`(OreKind/MineConfig/MineReceipt)、`schemas`(mine_config/mine_state/mine_regen/ore_balance/miner_nonce/emitted_this_epoch/treasury)、`events`(mine_settled/mine_depleted/mine_regened/ore_redeemed)、`errors`、`systems`。
2. `pnpm dubhe schemagen` 生成 Move 模块；核对生成物。
3. 写系统逻辑（DESIGN §3.2）：
   - `mine_system::mine_batch`：nonce 校验 → 收费入 treasury → `maybe_regen` → 逐次扣矿+`sui::random` roll 产出（沿用 `hit_rate=25`/`drop_rate=10` 语义）→ 排放封顶 assert → 落库（mine_state/ore_balance/nonce）→ emit `mine_settled`(+`mine_depleted`)。
   - `redeem_system::redeem`：销毁/转移 `ore_balance` → emit `ore_redeemed`。
   - `admin_system`：set 治理参数（**先留接口，初值在 M5 拍板前用安全占位/仅 admin 可调**）。
4. Move 单测：nonce 重放被拒、收费不足被拒、排放封顶被拒、再生、产出落库、事件字段正确。
5. 部署 testnet（`dubhe publish` / `sui client publish`），记录 `packageId` + 关键 object IDs。
6. TS 烟测（`@0xobelisk/sui-client`）：构造并发送 `mine_batch`，`parseState` 读回 `ore_balance`/`mine_state`，断言事件。

**File targets**：`onchain/dubhe.config.ts`、`onchain/contracts/mir2_mine/sources/{mine_system,redeem_system,admin_system}.move`（+ schemagen 产物）、`onchain/contracts/mir2_mine/tests/**`、`onchain/scripts/smoke-mine.ts`。

**Deliverables**：testnet `packageId`、部署 tx digest、一笔 `mine_batch` 的 tx digest + `mine_settled` 事件内容、Move 单测全绿、TS 烟测输出。

**Exit 出口判据**
- [ ] `sui move test` 全绿，覆盖 §3.2 全部不变量（nonce/费用/封顶/再生/产出/事件）。
- [ ] 合约部署 testnet，`packageId` 可复现记录。
- [ ] TS 烟测：成功发 `mine_batch`，链上 `ore_balance` 增加、`mine_state` 递减、`mine_settled` 事件字段正确；`nonce+1` 强制成立（重放被拒）。
- [ ] **未在合约里硬编码经济初值**（费率/封顶/兑率仅占位 + admin 可调，待 M5）。

**Dependencies**：M0。 **Risks**：① **配置 API 漂移（M0 已发现）**——DESIGN §3.1 基于旧 Dubhe API（`schemas`/`data`/`events`/`systems` + `storage()` 助手），但实装 SDK `@0xobelisk/sui-common@1.2.0-pre.96` 的 `DubheConfig` 用 `enums` / `components`(`{fields, keys?}`) / `resources` / `errors`，**无 `events`/`systems` 键**（事件由手写 Move 系统 emit）。**M1 必须把 §3.1 的 mine_config/mine_state/ore_balance/miner_nonce/emitted_this_epoch/treasury + OreKind + mine_settled… 翻译成实装 shape**（已在 `onchain/dubhe.config.ts` 标注）。② `sui::random` 用法（需 `&Random` + 两段式 commit/reveal）。③ schemagen 产物与手写系统耦合。④ shared object 测试。

---

## 5. M2 — Off-chain Bridge：Indexer + Relayer（WF-2, WF-3）∥ M3

> **Status（2026-06-09）：✅ DONE.** Dubhe indexer 跑通（WF-2，`pnpm indexer` → sqlite，验证在收 testnet 包事件）；Relayer（`onchain/relayer/`）读链事件 → `(tx_digest,event_seq)` 去重（幂等②，持久化、重启不丢不重）→ 归一化 `GrantOnchainOre` / `MineDepleted` / `CreditGoldFromOre`（`types.ts` = M2↔M3 契约；`miner`→`sui:0x..`；ore→gold 兑率留 M5）→ Log/HTTP sink。单测 **9/9**；**testnet 实测**：12 事件 → 3 命令（2 `GrantOnchainOre` + 1 `MineDepleted`），全量重放 → **0 命令 / 12 去重**（幂等②真数据验证）。**架构微调（已记）**：Relayer 直连 Sui `queryEvents(MoveModule{package, mine_system/redeem_system})` 取事件，而非 Dubhe GraphQL —— 该 SDK 的 indexer GraphQL 端点未文档化（随机端口），直连更稳且精确（只取本包），indexer 内部也是这么做；indexer 仍作 WF-2 保留（sqlite，供未来 GraphQL/前端）。

**Goal**：链上事件可被订阅、去重、归一化为"链确认事件"，并通过一个**注入接口**送达服务端（M3 提供接口；M2 先对 mock/admin-api 打通）。

**Tasks**
1. **Indexer**（WF-2）：`dubhe.config.json` + `dubhe-indexer --network testnet --with-graphql` 跑起来；确认 `mine_settled`/`ore_redeemed` 可在 GraphQL（`:4000/graphql`，订阅 `ws`）查询/订阅。
2. **Relayer**（WF-3）：新 TS 微服务 `onchain/relayer/`：
   - `createDubheGraphqlClient` 订阅 `mine_settled`/`ore_redeemed`。
   - **幂等②**：持久化已处理 `(tx_digest, event_seq)`，重复事件丢弃。
   - 归一化为内部命令（`GrantOnchainOre{account,ore_kind,amount,mine_id,stones_left}` / `CreditGoldFromOre{account,gold}`），带**幂等键**（= `tx_digest:event_seq`）。
   - 调用 gateway/admin-api 注入端点（M3 定义）；失败重试 + 死信。
   - `miner` 地址 → `account_id = "sui:0x.."` 映射（`auth.rs` 同构，无需绑定表）。

**File targets**：`onchain/indexer/{dubhe.config.json,run.sh}`、`onchain/relayer/{src/**,package.json,.env.example}`。

**Deliverables**：testnet 上发一笔 `mine_batch` → Indexer 收到 → Relayer 去重后产出一条归一化命令（先打到日志/mock 端点）+ 截图/日志。

**Exit 出口判据**
- [ ] Indexer 订阅到真实 testnet `mine_settled` 事件。
- [ ] Relayer 对**重复事件幂等**（同 `(tx_digest,event_seq)` 只产出一次）；断链重连不漏不重。
- [ ] 归一化命令 schema 与 M3 入站契约一致（联调前先 mock 校验）。

**Dependencies**：M1（要有真实事件）。 **Risks**：① **Indexer 原生依赖（M0 已发现）**——`@0xobelisk/sui-indexer` 传递依赖 `better-sqlite3@8.7.0`，在 **node 22 / darwin 编译失败**（V8 `SetAccessor` 已移除）→ M0 已将 indexer **移出** `onchain` 直装依赖；M2 改用 **Docker / 预编译 `dubhe-indexer` 二进制**，或 `pnpm override` 把 `better-sqlite3` 顶到 ≥11（带预编译产物），**勿作为 npm 原生依赖直接 install**。② GraphQL 订阅断连/回填。③ 事件顺序与 `event_seq` 语义。④ Relayer 信任边界（它能给玩家发矿 → M5/M6 收紧鉴权）。

---

## 6. M3 — Sim/Gateway Inbound（WF-4, WF-5）∥ M2

> **Status（2026-06-09）：✅ WF-4 DONE（WF-5 并入 M4）.** Sim 入站（commit bc0a22fad）：新增 `WorldCommand::GrantOnchainOre` / `CreditGoldFromOre`（= M2 relayer 的归一化命令）→ `apps/simulation/src/runtime/onchain.rs`：矿石入背包（镜像 P0 `give_mine_payout`，`OreKind`→`<Variant>Ore`，ore dura = units×1000）→ `GainedItem`；给金币（`can_gain_gold` 守卫）→ `GainedGold`。**幂等③**：`OnchainCommandLog` 资源记 `idempotency_key`（`tx_digest:event_seq`），重复严格 no-op。**与 P0 并存**（`mining.rs` 未动）；`validate_production_player_command` **拒绝玩家路径注入**（玩家不能凭空造矿/金）= sim 侧鉴权边界。单测 4 个绿（矿落库 dura=5000 / 重放 no-op / 金币幂等 / 玩家路径拒绝）+ **全 sim 套件绿（P0 不回归）** + `cargo fmt --all --check` 绿。**范围决策**：网关 HTTP 注入端点 + operator-token 鉴权 + 会话路由（WF-5）**并入 M4**（relayer↔gateway↔sim 合龙，届时端到端可测）；脱离 relayer 连接，该端点是无法测试的管线。

**Goal**：服务端能接收"链确认事件"并**权威落库**——矿石入背包、卖矿给金币、广播渲染对账——且**与 P0 并存**、**幂等③**。用 mock 注入驱动（不依赖真链，便于与 M2 并行）。

**Tasks**
1. **WF-4 Sim**：`world_runtime.rs` 新增 `WorldCommand` 变体：
   - `GrantOnchainOre{account, ore_kind, amount, mine_id, stones_left, idempotency_key}`
   - `CreditGoldFromOre{account, gold, idempotency_key}`
   - 在 `WorldCommandKind` 加对应判别；执行体把矿石写入背包 / 给金币 / 触发 `MineNodeState` 重广播（对账乐观值）。
   - **幂等③**：Sim 侧记录已处理 `idempotency_key`，重复命令 no-op。
   - **并存**：P0 `try_mine`/`give_mine_payout` 不动；新路径只在"链确认"时落库（乐观特效仍走 P0 心智）。
2. **WF-5 Gateway/Admin**：暴露**注入端点**（admin-api 内网 / gateway 受信通道），鉴权 operator token（**token 进 env**），校验后转 `WorldCommand`；`sui:0x` 身份核对（`auth.rs`）。
3. 出站（可选）：`GatewayGameplayEvent` 记审计/风控。

**File targets**：`apps/simulation/src/world_runtime.rs`、`runtime/mining.rs`（仅对账广播，不改 P0 扣减）、`config.rs`（如需把 mine_id↔mine_zone 映射）、`apps/gateway/src/{web.rs,events.rs,auth.rs}`、`apps/admin-api/**`。

**Deliverables**：单测——mock 注入 `GrantOnchainOre` → 背包矿石 +N、重复注入 no-op、`MineNodeState` 广播；`CreditGoldFromOre` → 金币 +N。

**Exit 出口判据**
- [ ] `cargo test -p mir2-simulation`（单线程）全绿，含新增幂等/落库/并存用例；**P0 既有挖矿用例不回归**。
- [ ] `cargo fmt --all --check` 绿。
- [ ] 重复 `idempotency_key` 命令严格 no-op（幂等③）。
- [ ] 注入端点鉴权（operator token from env）；未授权被拒。

**Dependencies**：可与 M2 并行（mock 注入）；契约需与 M2 归一化命令对齐。 **Risks**：背包/金币写入与 Crystal 语义一致性；并存路径不得双发（乐观 vs 链确认对账）。

---

## 7. M4 — End-to-End Vertical Slice（全 WF 合龙）

> **Status（2026-06-10）：◐ 后端脊柱 + 客户端纯逻辑核心 DONE；浏览器接线 + 实链 e2e 待真机。**
> **WF-5 网关注入脊柱（commit 08aec46be）**：受信 Relayer `POST /onchain/inject`（operator-token 鉴权，常量时间比较，生产 fail-closed）→ `LiveSessionInjector`（account_id→该 socket 任务的 mpsc 发送端，RAII 注册/注销）→ 目标活会话 `execute_with_outcome`（Direct，权威落库）→ 推包。`OnchainInjectCommand`（camelCase，对齐 relayer `types.ts`）：`GrantOnchainOre`/`CreditGoldFromOre`→`WorldCommand`，`MineDepleted`→渲染-only no-op。离线玩家 200 accepted/`connected:false`（持久化留 M6）。**幂等③仍在 Sim**（重复 `idempotency_key` no-op），网关无状态。门禁：`cargo fmt --all --check` 绿；网关 lib 套件 **268 passed / 0 failed**（新 9：3 operator-token + 6 inject）。
> **WF-6 客户端核心（commit 见 feat/onchain-mine）**：`apps/web/lib/onchain-mine.ts` —— PTB builders（`mine_batch` 从 gas split 出 `fee: Coin<SUI>`、7 参；`redeem` 先造 `OreKind` 再销毁）+ 攒挥批处理器 + 严格递增 nonce 跟踪（链同步不回退）+ 乐观↔链对账（phantom/shortfall delta）+ `stones_left`→矿脉分档（满/裂/空）；`onchain-mine-session.ts` —— 经 Wallet Standard `sui:signAndExecuteTransaction` 签发（复用 `passkey-auth.ts` 的钱包对象），只回 tx digest（矿/金经 Relayer→inject→Sim 权威落库，期间乐观 VFX）。门禁：`tsc --noEmit` 0 错；`test:frontend-logic` 绿（新 `test:onchain-mine` 14 组，builders 用 `Transaction.getData()` 断言，无需网络/钱包）。
> **待真机（非 headless 可验）**：`page.tsx` 接线（`harvestToward`→批处理器、`GainedItem`/`MineNodeState` case 对账、redeem UI、钱包连接）+ Relayer 真打注入端点的实链端到端冒烟（tx digest→背包→分档→redeem 得金币）。需浏览器 + 充值 testnet 钱包（CLAUDE.md 归 Codex 的部署/实链验证道）。**M4 出口的"端到端 + 三处幂等全生效"须在真机复核后才算闭合。**

**Goal**：DESIGN §4 全链路在 **testnet** 跑通：客户端攒 N 挥 → 1 笔 `mine_batch` → 链结算 → Indexer → Relayer（去重）→ Sim 权威落库 → 背包矿石 + `MineNodeState` 渲染分档；乐观特效与链确认**对账**一致。

**Tasks**
1. **WF-6 Client**：Sui 钱包连接（`account_id=sui:0x..` 复用）；挥镐攒批 → 构造 `mine_batch`（M5 前 gas 玩家自付即可，sponsored 留 M6）；挥击瞬间乐观 VFX（复用 P0 手感）；收到链确认（经服务端广播）后对账（多退少补）；按链上 `stones_left` 渲染满/裂/空。
2. 合龙 M1+M2+M3：Relayer 真打 M3 注入端点；端到端冒烟脚本。
3. `redeem`：客户端发 `redeem` → `ore_redeemed` → Relayer → `CreditGoldFromOre` → 金币到账（闭环）。

**File targets**：`apps/web/app/page.tsx`（`case "MineNodeState"` 对账 + 钱包/批量发包 + redeem UI）、`apps/web/lib/**`、`onchain/scripts/e2e-*.ts`。

**Deliverables**：一段**端到端 demo**（tx digest 链 → 背包矿石 → 渲染分档截图 → redeem 得金币）；对账正确性记录。

**Exit 出口判据**
- [ ] testnet 端到端：客户端发起 → 背包真实到矿 → 渲染按链上余量分档 → `redeem` 得金币。
- [ ] 乐观特效与链确认**最终一致**（对账无双发/无幻影矿）。
- [ ] 三处幂等全生效（nonce / `(tx_digest,event_seq)` / `idempotency_key`）。
- [ ] 全门禁绿（Move/Rust/TS）。**→ 停下汇报 E2E。**

**Dependencies**：M1, M2, M3。 **Risks**：钱包 UX；链延迟下的对账窗口；事件→广播链路时延。

---

## 8. M5 — Economy & Governance（WF-7）★进入前必须停下拍板

**Goal**：把经济参数从"占位"变成**链上治理参数**并定初值；落地反女巫/限速、treasury 对账。**这是 DESIGN §5/§6/§9 的开放决策——进入前把现状 + §5.3 算账 + 推荐默认值交付，等拍板再写死。**

**进入前必须拍板（见 §11 开放决策表）**：`PER_SWING_FEE`、批大小、`EPOCH_EMISSION_CAP`、`redeem` 兑率、每账号限速、gas 谁付（玩家/sponsor）、链目标与合规（是否可回兑→法务）。

**Tasks（拍板后）**
1. `admin_system` 参数化：费率/封顶/兑率/限速做成 admin 可调链上参数（治理 = 可信中立"央行"，DESIGN §5.0）。
2. 排放封顶 + 每账号 `(miner,epoch)` 限速硬上限（DESIGN §6）。
3. treasury 对账与提取流程；费入 treasury 可审计。
4. 反女巫门槛（钱包/账号绑定 `auth.rs`）。

**Exit 出口判据**
- [ ] 经济参数链上可治理、可审计；初值 = 拍板值。
- [ ] 限速 + 排放封顶在合约层强制；超限被拒（含测试）。
- [ ] treasury 余额/费用流可查。

**Dependencies**：M4。 **Risks**：定价错→通胀/挖矿死；合规（赌博/开箱式，若可回兑）。

---

## 9. M6 / M7 / M8 — Hardening, Server, Mainnet

### M6 — Hardening（全 WF）
- **热点对象分片**（DESIGN §6）：`mine_id#0..k` 子节点 / 每玩家累加 + 周期对账，缓解 Sui shared-object 串行。
- **Sponsored tx + 批量**：项目方代付 gas，玩家零 SUI 起步；批量进一步降争用。
- **对账健壮性**：链延迟/重组下乐观-权威一致性压测。
- **`refine_system`（可选，DESIGN §5.0+）**：链上精炼（消耗 `ore_balance`、`sui::random` 成功/炸装、`gear` 凭证、`gear_refined`/`gear_smashed`）；**战斗属性仍服务端权威**。
- **安全**：合约 + Relayer 信任边界审查；压测。
- **Exit**：热点矿压测达标；安全审查清单过；（如做）refine 闭环绿。

### M7 — Server / Staging Rollout（WF-5/7）★go/no-go
- 接真实（staging）gateway/sim 部署；运维监控（treasury/留存/矿价曲线）；小规模灰度。
- **不碰生产 gateway**；**进入前 go/no-go**。
- **Exit**：staging 端到端稳定运行；监控/告警就位；回滚预案。

### M8 — Mainnet（WF-1/7）★go
- 合约 + Relayer 信任边界**安全审计**；主网部署；风控接 Admin。
- **进入前明确 go**；**否则不上主网**。
- **Exit**：审计通过；主网部署可复现；风控/应急到位。

---

## 10. 跨切面关注点（Cross-Cutting，贯穿所有里程碑）

| 关注点 | 落地方式 | 里程碑 |
|---|---|---|
| **三处幂等** | ①`miner_nonce` 严格递增（M1）②Relayer `(tx_digest,event_seq)` 去重（M2）③Sim `idempotency_key` no-op（M3）；M4 三处同时验证 | M1–M4 |
| **服务端权威 / 乐观对账** | 链确认前只 VFX；落库只在 Relayer 注入后；`MineNodeState` 对账多退少补 | M3–M4 |
| **P0 并存** | `mining.rs`/`mine_zones`/`MineNodeState` 不改语义，仅叠加链路；P0 测试不回归 | M3+ |
| **密钥安全** | 私钥/助记词/operator token 只进 `onchain/.env`(gitignore) / 本地 keystore；`.env.example` 仅占位；CI 用 secrets | M0+ |
| **Testnet-only** | 全程 testnet；M8 前不碰主网；不碰生产 gateway | M0–M7 |
| **门禁** | 每里程碑末：`sui move test` + `cargo fmt --all --check` + `cargo test -p mir2-simulation`(单线程) + TS `typecheck`/`build` 全绿 | 每个 |
| **可审计** | treasury 链上公开；事件流可回放；Relayer 注入留痕 | M1+ |

---

## 11. 开放决策 & 推荐默认值（★M5 前必须拍板；现以占位/可调形态推进）

> **硬规则**：M5 前**不在合约里写死**任何经济初值；仅留 `admin_system` 可调接口 + 安全占位。下表为 DESIGN §5.3/§0.9 的**建议起点**，最终值需用户在 M5 gate 拍板。

| 决策 | DESIGN 依据 | 建议默认（待拍板） | 备注 |
|---|---|---|---|
| `PER_SWING_FEE` | §5.1/§5.3 | ≈ 0.004 SUI（gas≈0.0008 + 协议费≈0.0032） | 协议费入 treasury = 项目方收入 |
| 批大小 | §2-B | 25–50 挥/笔 | 按批内次数计费，收入不减 |
| `EPOCH_EMISSION_CAP` | §2-C | TBD（小放量起步看曲线） | 防矿石超发砸价 |
| `redeem` 兑率（矿石→金币） | §5.2/§5.3 | TBD（锚定"金币:SUI"，反推每份矿石定价） | 先做纯付费货币，无外部回兑 |
| 每账号限速 | §5.0-2/§6 | ≈ 3600/hr/account | 反女巫/反工作室承包水龙头 |
| gas 谁付 | §9-4 | M5 前玩家自付；M6 上 sponsored | 影响获客门槛 |
| 命中/掉落率 | 游戏 parity | `hit_rate=25` / `drop_rate=10` | 沿用 Crystal 语义 |
| 战力天花板 | §0.9 / §5.0+ | **赛季制温和抬顶**（已锁定） | 通胀关进节奏 |
| 升级装备流通 | §0.9 / §5.0+ | **可交易 + 交易行抽成**（已锁定），一级矿石成本略低于二级保 sink | 灵魂绑定 vs 可交易已选后者 |
| 炸装率/纯度/保底 | §5.0+ 主控杠杆 | TBD | 同一旋钮控 sink 深度/平权度 |
| 矿区选址 | 决策 E / §0.9 | **两层都做**；P0 取舍 A（比奇合成安全矿先可玩）vs B（导入 DeadMine） 倾向 A | DeadMine 导入并行排期 |
| 链目标 / 合规 | §9-5/§9-6 | testnet→（go 后）Sui 主网；可回兑需法务 | §6 热点优化是 Sui 专属 |

---

## 12. 与 DESIGN 分阶段（§8 P0–P5）映射

| DESIGN | 本路线图 | 状态 |
|---|---|---|
| P0（`MineNodeState` + 三档贴图 + `mine_zones`） | 基线（M0 之前） | ✅ 已完成 |
| P1（合约 schema+`mine_batch`+`redeem` testnet + TS 跑通） | **M1** | 待做 |
| P2（Indexer + Relayer → 注入 Sim；钱包↔账号） | **M2 + M3 + M4** | 待做 |
| P3（`redeem`→金币；治理参数；sponsored+批量） | **M4(redeem) + M5 + M6** | 待做 |
| P4（小规模灰度调参；热点分片压测） | **M6 + M7** | 待做 |
| P5（主网；安全审计；风控接 Admin） | **M8** | 待做 |

---

## 13. 命令速查（onchain/）

```bash
# Move（合约）
cd mir2-web3/onchain/contracts/mir2_mine && sui move build && sui move test

# Dubhe schemagen（M1+）
cd mir2-web3/onchain && pnpm dubhe schemagen          # 由 dubhe.config.ts 生成 Move

# TS（烟测/脚本/relayer）
cd mir2-web3/onchain && pnpm install && pnpm typecheck && pnpm build

# Indexer（M2+）
dubhe-indexer --config dubhe.config.json --network testnet --with-graphql

# 钱包（M0；私钥永不入库）
sui client switch --env testnet
sui client active-address && sui client gas

# 服务端门禁（M3+，仓库根 mir2-web3/）
cargo fmt --all --check && cargo test -p mir2-simulation --locked -- --test-threads=1
```

---

## 附：参考

- 设计：[`ONCHAIN-SMART-MINE-DESIGN.md`](./ONCHAIN-SMART-MINE-DESIGN.md)（schema §3.1、`mine_batch` §3.2、时序 §4、经济 §5、并发 §6、渲染 §7、分阶段 §8、开放问题 §9）。
- Dubhe：https://github.com/0xobelisk/Dubhe（`examples/sui/{constantinople,dms}`、`framework`(Move)、`packages`(TS SDK)）；文档源码 `pages/dubhe/sui/*.mdx`。
- 关键包：`@0xobelisk/sui-common`(config)、`@0xobelisk/sui-client`(tx/parseState)、`@0xobelisk/sui-indexer`、`@0xobelisk/graphql-client`、`@0xobelisk/ecs`、`@0xobelisk/sui-cli`(`dubhe` CLI)。
