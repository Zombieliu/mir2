# 链上智能矿场设计（On-Chain Smart Mine, Dubhe / Sui）

> 状态：草案 v0.1 — 供讨论。本文把"玩家挖矿发交易 → 链结算 → 索引器订阅 → 通知游戏服务端 → 给玩家矿/渲染"这套流程，落到 mir2-web3 现有架构 + Obelisk **Dubhe** 引擎上。
>
> 目标读者：游戏服务端（Rust/`bevy_ecs`）、链上合约（Move/Dubhe）、前端（Next.js/Bevy）、经济/产品。

---

## 0. 一句话结论

- **渲染（矿越挖越少看得见）**：好实现，服务端已有 `stones_left` 状态，只缺一个广播包 + 客户端贴图切换，纯增量。
- **链上矿场**：技术上完全可行，且 **Dubhe 的契合度非常高**（Move 版 ECS ↔ 你的 `bevy_ecs`，自带 Indexer+GraphQL 订阅 = 你要的"索引器通知服务端"那一环，Sui 钱包已经是账号）。
- **经济模型（挖 1000 次≈4 SUI、项目方靠调用收费、玩家卖矿换金币）**：可以挣钱，但**有三处必须改造**，否则会被延迟、Sui 热点对象并发、和经济不可持续三件事咬到（见 §2、§5）。

---

## 1. 现状盘点（两套 ECS 世界）

### 1.1 游戏侧已有的"接缝"（重要：80% 的集成肌肉已就位）

| 能力 | 现有实现 | 位置 |
|---|---|---|
| 权威挖矿逻辑 + 每矿点剩余量 | `MineSpot { stones_left, mine_set_index, last_regen_tick }`、`try_mine`、`give_mine_payout`、确定性 `roll()` | `apps/simulation/src/runtime/mining.rs` |
| 矿区数据结构 | `MineZoneRecord { map_file_name, mine_set, x, y, size }` | `apps/simulation/src/config.rs:2118` |
| **链→服务端指令注入口** | `WorldCommand` / `WorldCommandKind`（已有字符串式 `Stage5Command(String)` 先例，可扩展） | `apps/simulation/src/world_runtime.rs:70` |
| **服务端→外部事件总线** | `GatewayGameplayEvent` + 可插拔 `EventPublisher`（已有 Redpanda 实现），topic `gameplay.command.executed` | `apps/gateway/src/events.rs` |
| **钱包↔账号绑定** | `account_id` 本身即 `"sui:0x..."` | `apps/gateway/src/auth.rs:14` |
| 客户端收包/渲染入口 | packet `switch`（`case "MapEffect"` 等） | `apps/web/app/page.tsx:5446` |

> 结论：服务端→链（出站）和链→服务端（入站）两条路都已有同构的现成机制，不需要从零造管线。

### 1.2 Dubhe 提供的对应件

Dubhe 是社区开源的 **Move 应用/全链游戏引擎**，"Harvard 架构 + Schema 存储"，可类比为 **Move 版 MUD**。核心原语：

- **Schema = 组件（数据）**，**System = 系统（逻辑）**，链上对象 ID ≈ 实体，通过 dynamic field 把组件挂到实体上 —— 与 `bevy_ecs` 心智模型一致。
- 存储原语：`StorageValue<T>`（单值）、`StorageMap<K,V>`、`StorageDoubleMap<K1,K2,V>`。
- 声明式配置 `dubhe.config.ts` 定义 `data`（结构/枚举）、`schemas`（组件）、`events`、`errors`、`systems`；跑 `pnpm dubhe schemagen` 自动生成 Move 模块与事件类型。
- **Dubhe Indexer**：`dubhe-indexer --config dubhe.config.json --network testnet --with-graphql`，订阅链上事件流 → GraphQL（`http://localhost:4000/graphql`，订阅 `ws://localhost:4000/graphql`，`ENABLE_SUBSCRIPTIONS`）。**这正是"索引器订阅通知服务端"那一环，开箱即用。**
- **TS Client**：`new Dubhe({ networkType, packageId, metadata, secretKey })`；调用系统 `dubhe.tx.<system>.<fn>({ tx, params, onSuccess, onError })`；读状态 `dubhe.parseState({ schema, objectId, storageType, params })`。
- 参考示例 `examples/sui/constantinople`（链上地图 + 坐标 + 实体 + 遭遇事件）几乎就是"地图矿场"的模板。
- 多链（Sui 主、Aptos/Rooch/Initia）。

---

## 2. 关键决策（先拍板，再写合约）

这几个决定直接决定合约和经济长什么样，**必须先定**：

### 决策 A：链 vs 服务端，谁是矿/矿石的真相源？

你的收费模型要求"挖矿动作必须上链才能收费"，所以本设计采用：

- **链权威**：矿点剩余量、矿石所有权、产出随机数 —— 都以链上为准（这样收费、稀缺、可交易才成立）。
- **服务端只做"乐观预演 + 渲染镜像"**：挥镐瞬间服务端先播特效/提示（手感），真实矿石余额由链经索引器异步回写。
- **绝不允许**两边各自扣减矿量 → 否则必然对不上账（幻影矿/双花）。

### 决策 B：逐挥上链 vs 批量结算（强烈建议批量）

"挖 1000 次 = 1000 笔交易"会撞上延迟、gas、吞吐三堵墙。建议：

- 客户端/服务端把 **N 次挥镐攒成 1 笔 `mine_batch` 交易**（如 25~50 次/笔），交易里带"本批挥击次数 + 防重放 nonce"。
- 收费按 **批内次数** 计（1 笔交易收 50 次的费），所以批量**不减少收入**，只砍掉 95%+ 的交易开销。
- 把"项目方收益"从"原始 gas"**解耦成显式协议费**（见 §5），交易可由项目方 **sponsored transaction** 代付 gas、再从费用里抽成 —— 玩家甚至可以零 SUI 余额起步。

### 决策 C：有限枯竭 vs 可再生（建议"可再生节点 + 受控排放"）

- **节点可再生**（地图上始终有矿可挖，保住 UX，沿用现有 5 分钟 regen 心智）。
- 但**矿石进入流通的总量受链上排放表/每 epoch 上限约束**（保住代币经济，不让矿石无限通胀砸价）。
- 即：把"有没有矿可敲"（可再生，UX）与"放出多少矿石"（封顶，经济）**分离**。

### 决策 D：随机数（建议用 Sui 链上随机，正好是 Dubhe "provable" 卖点）

- 产出 roll 在链上用 `sui::random`（Sui 原生可验证随机）→ 产出**可证明、去信任**。
- 服务端的确定性 `roll()` 降级为"乐观预览"，最终以链上为准。

---

## 3. 链上层：Dubhe Schema 设计

### 3.1 `dubhe.config.ts`（矿场组件 + 系统 + 事件）

```ts
import { DubheConfig } from '@0xobelisk/sui-common';

export const dubheConfig = {
  name: 'mir2_mine',
  description: 'MIR2 on-chain smart mine',

  // —— 自定义数据结构/枚举（组件字段会引用它们）——
  data: {
    OreKind: ['BlackIron', 'Gold', 'Silver', 'Copper',
              'Platinum', 'Ruby', 'Nephrite', 'Amethyst'],
    // 一座矿的静态配置（对应游戏里的 MineZoneRecord + MineSet）
    MineConfig: {
      map_id: 'u32',        // 哪张地图
      mine_set: 'u8',       // 1 或 2，决定产出表
      max_stones: 'u32',    // 满库存
      regen_secs: 'u32',    // 再生周期
      hit_rate: 'u8',       // 命中率(=25)
      drop_rate: 'u8',      // 掉落率(=10)
    },
    // 一次批量挖矿的链上结算结果（回放给索引器/服务端）
    MineReceipt: {
      miner: 'address',
      mine_id: 'u64',
      swings: 'u16',        // 本批挥击次数
      ore_kind: 'OreKind',
      ore_amount: 'u64',    // 本批产出（= 游戏里的"纯度/数量"聚合）
      stones_left: 'u32',   // 结算后矿点剩余 → 驱动渲染分档
    },
  },

  schemas: {
    // 全局：矿配置表（mine_id -> MineConfig）
    mine_config: storage('u64', 'MineConfig'),
    // 全局：每座矿的动态剩余量 + 下次再生时间（mine_id -> ...）
    mine_state:  storage('u64', 'u32'),     // mine_id -> stones_left
    mine_regen:  storage('u64', 'u64'),     // mine_id -> next_regen_ms
    // 每个玩家每种矿石的余额（(miner, OreKind) -> amount）
    ore_balance: storage('address', 'OreKind', 'u64'), // StorageDoubleMap
    // 防重放：玩家已结算的最大 nonce
    miner_nonce: storage('address', 'u64'),
    // 排放治理：本 epoch 已放出的矿石总量（受 §2-C 上限约束）
    emitted_this_epoch: storage('u64'),     // StorageValue
    // 协议金库累计（SUI）
    treasury: storage('u64'),               // StorageValue
  },

  events: {
    // schemagen 会自动为每个 schema 生成 set/remove 事件；
    // 这里再声明业务事件，供 Indexer 订阅、服务端消费：
    mine_settled: {                          // 一批挖矿结算完成
      miner: 'address', mine_id: 'u64',
      swings: 'u16', ore_kind: 'OreKind',
      ore_amount: 'u64', stones_left: 'u32',
      fee_paid: 'u64', nonce: 'u64',
    },
    mine_depleted: { mine_id: 'u64' },       // 某矿枯竭（渲染→空岩）
    mine_regened:  { mine_id: 'u64', stones_left: 'u32' }, // 再生回满
    ore_redeemed:  { miner: 'address', ore_kind: 'OreKind', amount: 'u64' }, // 卖矿换金币（链上销毁）
  },

  errors: {
    mine_not_found: 'Mine not found',
    mine_exhausted: 'Mine has no stones left',
    bad_nonce: 'Replay or out-of-order nonce',
    insufficient_fee: 'Attached SUI is less than required fee',
    emission_cap_reached: 'Epoch emission cap reached',
  },

  systems: ['mine_system', 'redeem_system', 'admin_system'],
} as DubheConfig;
```

> `storage('a')` → `StorageValue<a>`；`storage('k','v')` → `StorageMap<k,v>`；`storage('k1','k2','v')` → `StorageDoubleMap`。跑 `pnpm dubhe schemagen` 生成 Move。

### 3.2 核心系统 `mine_system::mine_batch`（Move 伪代码）

```move
/// 玩家一笔交易结算 N 次挥镐。链是权威：扣矿量、roll 产出、收协议费、记余额、发事件。
public entry fun mine_batch(
    mine_id: u64,
    swings: u16,
    nonce: u64,
    fee: Coin<SUI>,                 // 玩家(或项目方 sponsor)附带的费用
    rnd: &sui::random::Random,      // 链上可验证随机（决策 D）
    clock: &Clock,
    ctx: &mut TxContext,
) {
    let miner = tx_context::sender(ctx);

    // 1) 防重放
    assert!(nonce == miner_nonce(miner) + 1, E_BAD_NONCE);

    // 2) 收费 → 金库（决策 B：按批内 swings 计费；项目方收益来源）
    let required = fee_for_swings(swings);            // 见 §5 费率
    assert!(coin::value(&fee) >= required, E_INSUFFICIENT_FEE);
    treasury_deposit(fee);

    // 3) 再生检查（决策 C：节点可再生）
    maybe_regen(mine_id, clock);

    // 4) 逐次/聚合扣矿 + roll 产出（沿用游戏 hit_rate/drop_rate/产出表语义）
    let mut left = mine_state(mine_id);
    let mut ore: u64 = 0;
    let cfg = mine_config(mine_id);
    let mut i = 0;
    while (i < swings && left > 0) {
        left = left - 1;
        if (roll_u8(rnd, ctx) < cfg.hit_rate &&
            roll_u8(rnd, ctx) < cfg.drop_rate) {
            ore = ore + roll_ore_amount(rnd, cfg, ctx); // = 游戏里的"纯度*1000"语义
        };
        i = i + 1;
    };

    // 5) 排放上限（决策 C：经济封顶）
    assert!(emitted_this_epoch() + ore <= EPOCH_EMISSION_CAP, E_EMISSION_CAP);
    add_emitted(ore);

    // 6) 落库：矿点剩余、玩家矿石余额、nonce
    set_mine_state(mine_id, left);
    let kind = ore_kind_of(cfg.mine_set);
    add_ore_balance(miner, kind, ore);
    set_miner_nonce(miner, nonce);

    // 7) 发事件（Indexer 订阅 → 服务端消费）
    emit mine_settled { miner, mine_id, swings, ore_kind: kind,
                        ore_amount: ore, stones_left: left,
                        fee_paid: required, nonce };
    if (left == 0) emit mine_depleted { mine_id };
}
```

`redeem_system::redeem`：玩家把链上矿石**销毁/转入金库**换游戏金币 —— 发 `ore_redeemed` 事件，由索引器→服务端给玩家加金币（§4）。

---

## 4. 集成架构（端到端时序）

```
┌──────────┐   挥镐(普通近战,无技能)    ┌─────────────┐
│  客户端   │ ─────────────────────────▶ │  Gateway/Sim │  ① 服务端乐观反馈
│ Next/Bevy│ ◀── MapEffect粉尘 + 矿石提示(乐观) ─ │ (mining.rs) │     立即播特效,体验不等链
└────┬─────┘                            └──────┬──────┘
     │ 攒满 N 次 → 发 1 笔 mine_batch 交易        │ ②(可选)出站事件
     ▼                                          │   GatewayGameplayEvent
┌──────────┐   mine_batch(Coin<SUI>)            ▼   → 审计/风控/排行
│ Sui +    │ ── 扣矿量·roll产出·收费·发事件 ──▶ emit mine_settled / mine_depleted
│ Dubhe合约│                                          │
└────┬─────┘                                          │
     │ ③ 链上事件流                                    │
     ▼                                                │
┌──────────────┐  GraphQL 订阅(ws)  ┌───────────────┐ │
│ Dubhe Indexer│ ─────────────────▶ │ Relayer/Bridge │ │  ④ 入站
└──────────────┘                    │  (新增小服务)   │ │
                                    └───────┬───────┘ │
                                            │ 转成 WorldCommand::GrantOnchainOre
                                            ▼                 (world_runtime.rs:70 新增变体)
                                    ┌─────────────┐
                                    │   Sim 权威   │ ⑤ 把"链确认的矿石"落到背包,
                                    │ (bevy_ecs)  │    并广播 MineNodeState{stage}
                                    └──────┬──────┘    对账/修正乐观结果
                                           ▼
                                    ┌──────────┐  ⑥ 客户端按 stages_left 分档换贴图
                                    │  客户端   │     满矿脉→裂开→空岩
                                    └──────────┘
```

落到代码的接缝：

- **出站（可选，§4-②）**：复用 `EventPublisher`（`events.rs`）做审计/风控/排行；挖矿真相不依赖它。
- **入站桥（§4-④⑤）**：新增 **Relayer** 微服务，用 `createDubheGraphqlClient` 订阅 `mine_settled`/`ore_redeemed` → 调 gateway/admin-api → 注入新 `WorldCommand`：
  - `GrantOnchainOre { account: "sui:0x..", ore_kind, amount, mine_id, stones_left }`
  - `CreditGoldFromOre { account, gold }`（卖矿换金币）
  在 `WorldCommandKind`（`world_runtime.rs:70`）加对应变体，模拟器里把矿石写进背包 / 给金币 / 广播渲染。
- **身份**：`account_id == "sui:0x.."`（`auth.rs:14`），链上 `miner` 地址直接映射玩家，无需额外绑定表。

---

## 5. 经济模型 —— 你这套，我怎么看

### 5.0 设计原理：SUI 门槛水龙头 = 可信中立的金币价格上限（反垄断）

核心命题（产品方提出，本设计认同其内核）：

> 休闲玩家的金币需求很小（买药/修理/任务/打怪即可自给）；**大额金币需求集中在武器精炼、高端装备、大宗交易**，过去只能找玩家商人，而商人**容易控货、单向垄断金币市场**。引入一个**以 SUI 计价的挖矿金币水龙头**（如 3600 次/小时 ≈ 10 SUI ≈ $10），让任何人都能用固定真金价把 SUI→矿石→金币，从而**给商人金币报价设一个价格天花板**，打破单向垄断；同时高级矿石可**精炼武器（自用或卖出）**，提供效用与博弈上行。

为什么这个机制成立、且为什么值得上链：

- **价格天花板逻辑（成立）**：存在固定 SUI 价的替代供给后，没人会以高于挖矿价向商人买金币 → 商人金币价被封顶在挖矿价。这是用替代供给设价格上限的标准手法。
- **反通胀（优于普通水龙头）**：传统 MMO "打怪→免费金币→通胀"；此处**每放出金币都要真烧 SUI**，金币发行量锚定真实成本。
- **矿石双效用（补上 sink）**：矿石既能换金币（水龙头），又能**精炼烧掉**（`BlackIronOre` 纯度→升级成功率，`packets.rs:3351`/`:3438`；失败炸武器=装备 sink）。矿石因此有内在效用，不只是水龙头燃料。
- **为什么必须上链 = 可信中立**：水龙头汇率/爆率若是服务端私有旋钮，玩家无法信任项目方不偷印金币、不暗改爆率。做成**链上合约 + 治理参数 + `sui::random` 可证明随机**，这个"央行"才可信中立，价格天花板才可信。**这是本方案必须上链的最强理由**（强于"矿石可交易"）。

**但该原理引入四个必须管理的约束（否则反被反噬）：**

1. **项目方成了央行**：水龙头把金币锚定 SUI，必须**持续正确定价/调参**；放多→通胀，放少→挖矿死、商人夺回定价。是把货币政策从商人手里接过来，不是消灭它。→ 用 §2-C 排放上限 + `admin_system` 治理参数管理。
2. **商人会变成最大矿工**：有 SUI 资本者可规模化挖（多号 × $10/hr）。价格天花板仍守住，但**出货量会向资本集中**。→ **每账号挖矿硬限速（如 3600/hr）+ 反女巫/反脚本是必需项**（见 §6）。
3. **"概率赚钱"在数学上是赌场**：协议费 = house edge，**全体玩家必然净亏**（赢家的钱来自输家+抽水）。可定位为"好玩+博一把+有用"，但**不可宣传"挖矿赚钱"**；若矿石可回兑 SUI，存在赌博/开箱式合规风险，需法务评估。
4. **挖矿价必须高于"打怪刷金币"地板**：否则没人玩 PvE 刷钱，游戏经济空心化。**水龙头应是地板之上的付费便利上限，不是抄底**。

### 5.1 你描述的模型

> 玩家挖 1000 次 ≈ 4 SUI 链上成本；项目方"通过调用挣钱"；玩家得矿 → 卖成（游戏）金币。

拆开看：≈ **0.004 SUI / 次**。按 §2-B 批量化后，真实 gas 远低于此，**0.004 SUI 里大部分应是"协议费"而非 gas** —— 这恰好就是项目方收益。即把它设计成：

```
玩家每次挖矿支付 fee = gas(由项目方 sponsor) + 协议费(进 treasury)
fee_for_swings(n) = n * PER_SWING_FEE          // PER_SWING_FEE ≈ 0.004 SUI
项目方收入 ≈ Σ 协议费；玩家产出 = 矿石 → 卖成金币
```

### 5.2 可行性判断：能挣钱，但闭环取决于"矿石→金币→价值"

| 维度 | 评价 |
|---|---|
| 收入可预测 | ✅ 协议费按挖矿次数线性，清晰、链上透明、可审计（treasury 余额公开） |
| 复用现成基建 | ✅ Dubhe Indexer/GraphQL + 你的 WorldCommand/事件总线，几乎不用造管线 |
| SUI sink / 矿石 source | ✅ 形成"花 SUI → 得矿 → 换金币"的双币循环 |
| **闭环可持续性** | ⚠️ **最大风险**：见下 |

**核心问题：玩家花 4 SUI 挖到的矿，卖成金币后值不值 4 SUI？**

- 若 **矿石→金币的价值 < 4 SUI**：玩家净亏，本质是"付费游玩(pay-to-play)"，靠新鲜感撑不久 → 留存崩。
- 若 **> 4 SUI**：项目方在补贴，或金币在通胀 → 要么亏钱，要么砸盘。
- 只有当"金币有真实消耗去向（sink）"且**矿石排放受控（§2-C）**时，价格才稳。

**因此必须先回答："金币的价值从哪来？"**
1. 纯服务端货币、无外部价值 → 这套等于"**用 SUI 购买游戏金币**"（变相充值），完全合法且好赚，但要诚实定位成 **pay-for-currency**，不是 play-to-earn，别承诺玩家"挖矿致富"。
2. 金币可换装备/精炼/交易行且有强 sink → 可形成内循环经济，矿石才有承接。
3. 矿石/金币可在二级市场换回 SUI/稳定币 → 进入真正的 GameFi，**但此时"新玩家的 SUI 给老玩家发钱"的庞氏风险与合规问题随之而来**，需要真实需求（sink）兜底，不能只靠拉新。

### 5.3 一个算账示例（务必自己代真实数填一遍）

```
设 PER_SWING_FEE = 0.004 SUI，其中 gas ≈ 0.0008、协议费 ≈ 0.0032
1000 次挖矿：玩家付 4 SUI；项目方净收 ≈ 3.2 SUI；gas 成本 ≈ 0.8 SUI
1000 次产出（按 hit 25% × drop 10% ≈ 2.5% 命中产矿）≈ 25 次出矿
→ 要让玩家不亏，这 25 份矿卖出的金币必须 ≥ 4 SUI 等值
→ 反推"金币:SUI 锚定"与"每份矿石金币定价"，并设矿石排放上限防超发
```

> 建议：把 `PER_SWING_FEE`、`EPOCH_EMISSION_CAP`、矿石→金币兑率都做成 `admin_system` 可调的链上治理参数，**先小规模放量、看真实留存与 treasury 曲线再调**。

### 5.4 我的三条硬建议（已内置进上面的设计）

1. **批量上链 + 协议费与 gas 解耦**（§2-B、§5.1）：别真发 1000 笔；1 笔结算 25~50 次，sponsor gas、显式收费。收入不减，体验和吞吐天差地别。
2. **解决 Sui 热点对象并发**（§6）：单座热门矿是 shared object，会串行化。用"矿分片 / 每玩家累加 + 周期对账"避免人人争抢同一个对象。
3. **经济可持续优先于"纯链上"**：先定金币价值来源与 sink，矿石排放上链封顶；起步用"链做所有权+稀缺账本、服务端做手感"的偏中心化形态，别为去中心化牺牲体验和经济稳定。

---

## 6. 权威 / 防作弊 / 并发（Sui 特性）

- **热点共享对象**：`mine_state(mine_id)` 是共享状态，高并发挖同一座矿会让交易串行/重试失败。缓解：
  - **矿分片**：一座矿拆成 `mine_id#0..k` 多个子节点，客户端散列到不同子节点；
  - **每玩家累加 + 周期对账**：玩家挖矿先记到自己的 owned 对象（走 Sui 快速路径，无需共识排序），定期把"总消耗"对账回全局矿量；
  - 配合 sponsored / 批量，进一步降低争用频率。
- **防重放**：`miner_nonce` 严格递增（§3.2）。
- **随机可证明**：`sui::random`，杜绝"服务端偷偷调产出"质疑。
- **乐观结算对账**：服务端乐观给的矿石仅作 UI 预览；以 `mine_settled` 为准修正（多退少补），避免乐观与链上不一致。
- **双花/幻影矿**：唯一真相源在链（决策 A），服务端永不独立扣矿量。
- **每账号限速 + 反女巫（§5.0-2 必需项）**：链上对 `(miner, epoch)` 设挖矿次数硬上限（如 3600/hr），防资本/脚本规模化承包水龙头；配合身份门槛（钱包年龄/质押/与游戏账号绑定 `auth.rs:14`）抬高女巫成本。否则"休闲 vs 鲸鱼"分层与价格上限的公平性都会被工作室击穿。

---

## 7. 渲染（矿越挖越少）

- 服务端已有 `stones_left`；新增广播包 `MineNodeState { location, mine_id, stage }`，`stage = f(stones_left / max_stones)`（满/裂/空三档够用）。
- 触发点：① `mine_settled` 经 Relayer 回写后；② `mine_depleted`/`mine_regened` 时。
- 客户端在 `apps/web/app/page.tsx` 的 packet `switch`（`:5446`）加 `case "MineNodeState"`，按 stage 在该格叠矿脉 sprite。
- **真正成本是美术**（三档矿脉贴图），网络与状态都现成。
- 加分项：stage 由**链上剩余量**驱动 → 链上的稀缺**肉眼可见**，正是智能矿场的卖点。

---

## 8. 分阶段路线（MVP → 上链）

| 阶段 | 内容 | 是否碰链 |
|---|---|---|
| P0 | `MineNodeState` 广播 + 客户端三档贴图；矿区数据先填进 `config.mine_zones`（纯游戏侧"越挖越少"可玩可见） | 否 |
| P1 | Dubhe 合约 `mir2_mine`（schema+`mine_batch`+`redeem`）部署到 **testnet**；TS client 跑通一笔 `mine_batch` | 是(测试网) |
| P2 | Dubhe Indexer + Relayer 微服务：订阅 `mine_settled` → `GrantOnchainOre` 注入 Sim；钱包地址↔账号打通 | 是 |
| P3 | 卖矿换金币 `redeem` → `CreditGoldFromOre`；协议费/排放上限/兑率做成治理参数；sponsored tx + 批量 | 是 |
| P4 | 经济小规模放量灰度，盯 treasury/留存/矿石价格曲线调参；热点矿分片压测 | 是 |
| P5 | 主网；安全审计（合约 + Relayer 信任边界）；风控接 Admin | 是(主网) |

---

## 9. 开放问题（需产品/经济拍板）

1. **金币价值来源 + sink？**（决定这是 pay-for-currency 还是 GameFi，§5.2）
2. 矿是**可再生**还是**有限枯竭**？排放上限怎么定？（§2-C）
3. `PER_SWING_FEE` / 批大小 / 兑率初值？（§5.3）
4. gas 由**玩家付**还是**项目方 sponsor**？（影响是否需要玩家有 SUI 余额、获客门槛）
5. 链选型：Sui 主网，还是先 Sui testnet / Movement？（Dubhe 多链，但 §6 热点对象优化是 Sui 专属）
6. 合规：矿石/金币是否可换回真实价值资产？若是，需法务介入（§5.0-3 赌博/开箱式风险）。
7. **每账号挖矿限速值 + 反女巫门槛**怎么定？（§5.0-2、§6）决定水龙头是否会被工作室承包。
8. **挖矿金币价 vs 打怪刷金币地板**的校准：如何保证挖矿是"地板之上的付费便利"而非抄底，避免空心化 PvE 经济？（§5.0-4）

---

## 附录：Dubhe 参考

- 仓库：https://github.com/0xobelisk/Dubhe — `crates`(Rust)/`packages`(TS SDK)/`framework`(Move)/`templates`/`examples`(sui: `constantinople`,`dms`)
- 文档源码：https://github.com/0xobelisk/dubhe-docs（站点 `dubhe-docs.obelisk.build` 对抓取器 403，看 repo 内 `pages/dubhe/sui/*.mdx`）
- 关键包：`@0xobelisk/sui-common`(config)、`@0xobelisk/sui-client`(tx/parseState)、`@0xobelisk/sui-indexer`、`@0xobelisk/graphql-client`、`@0xobelisk/ecs`
- 命令：`pnpm dubhe schemagen`、`dubhe-indexer --config dubhe.config.json --network testnet --with-graphql`
- 概念锚点：Schema=组件、System=逻辑、Indexer 订阅事件→GraphQL（`http://localhost:4000/graphql`，订阅 `ws://localhost:4000/graphql`）
