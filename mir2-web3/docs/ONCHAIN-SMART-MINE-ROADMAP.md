# 链上智能矿场 —— 端到端实施路线图（全量落地）

> 配套设计:`docs/ONCHAIN-SMART-MINE-DESIGN.md`（架构、Dubhe schema §3.1、`mine_batch` §3.2、集成时序 §4、锁定决策 §0.9、经济 §5）。
> 本文是“从现状一路做到生产可用”的**实施**路线图:把设计拆成可执行里程碑 + 工作流 + 出口判据。
> 状态基线（2026-06-09）:P0 服务端挖矿 + `MineNodeState` 渲染广播**已实现**;链上(合约/索引器/Relayer)**为零**;唯一链接触点是 Sui 登录。

## 0. 范围与“完工”定义（Definition of Done）

**完工 = 玩家在 web 客户端挖矿 → 钱包签一笔 `mine_batch` → Sui 链权威结算 → 索引器→Relayer→服务端把链确认的矿石写进背包并修正渲染 → 玩家 `redeem` 卖矿换金币 → 金币买/升级可交易装备**,且:

- 全链路在 **Sui mainnet** 跑通、被监控、可灰度/回滚;
- 合约 + Relayer 通过**安全审计**;
- 服务端始终是游戏权威(链只做矿/矿石的结算真相源),P0 服务端挖矿与链上路径**并存不破**;
- 经济参数(费率/批大小/排放上限/兑率)**可治理**,有 treasury/留存/矿价看板。

**非目标(明确排除)**:把全部游戏物品上链;外部法币回兑(锁定决策②先做纯付费货币);跨链(先 Sui,Movement 留作后续)。

## 1. 指导原则(每个里程碑都受其约束)

1. **服务端权威**:链确认前只做乐观特效;真正发矿/给金币只在 Relayer 注入“链确认事件”后,由 Sim 落库。永不让客户端自报矿石。
2. **批量结算**:攒 N 挥发一笔 `mine_batch`,不逐挥上链(延迟/Gas/热点对象三杀)。
3. **幂等 + 防重放**:链上 `nonce`;Relayer 用 `(tx_digest, event_seq)` 去重;Sim 入站命令带幂等键。
4. **链上可验证随机**:`sui::random::Random`(锁定决策 D)。
5. **可再生 + 排放封顶**:节点再生 + `EPOCH_EMISSION_CAP`(决策 C),保经济可持续。
6. **安全先于上量**:mainnet 前必过审计 + 负载 + reorg/最终性边界测试。
7. **可观测 + 可回滚**:每层有指标、有 kill-switch、有回退路径。

## 2. 七条工作流(Workstreams / Lanes)

| Lane | 内容 | 技术栈 | 落点 |
|---|---|---|---|
| **L1 合约** | Dubhe schema + `mine_batch`/`redeem`/`admin` + 测试 + 部署 | Move / Dubhe / Sui | `onchain/mir2-mine/` |
| **L2 索引器** | Dubhe Indexer + GraphQL 订阅 | Dubhe Indexer | `onchain/indexer/`(配置) |
| **L3 Relayer** | 订阅事件→去重/排序/重试→调服务端 | TS(`@0xobelisk/graphql-client`) | `onchain/relayer/` |
| **L4 服务端/Sim** | 入站 `WorldCommand` + admin 入站端点 + 对账 | Rust / `bevy_ecs` | `apps/simulation/`、`apps/gateway/` |
| **L5 客户端** | 发交易/签名/sponsored、对账、redeem UI、矿石/金币展示 | Next / Bevy / `@mysten/sui` | `apps/web/` |
| **L6 经济/治理** | 费率/批大小/排放/兑率/赛季抬顶/交易行抽成 | 合约 admin + 配置 | L1 + `config` |
| **L7 运维/安全** | 部署(systemd)、监控看板、负载/热点分片、审计、风控 | systemd / 监控 / 审计 | `infra/`、`docs/` |

## 3. 里程碑(M0 → M8)

> 规模:S≈1–3 天、M≈1 周、L≈2–3 周(单人量级,供排期参考,非日历承诺)。每个里程碑都有**出口判据(Exit)**,达成才进下一阶段。

### M0 — 地基与工具链(不碰链) · 规模 S

- **L1**:建 `onchain/` 独立工作区(Move+TS,**勿混入 Rust workspace / pnpm root**);Dubhe CLI、Sui CLI 安装;testnet 钱包 + faucet 充值;`onchain/.env.example`(packageId/network/keys 占位,私钥进 `.gitignore`)。
- **L7**:`onchain/` 接 CI(Move build/test + TS lint);仓库根 `.gitignore`/`.vercelignore` 排除 `onchain/` 产物与密钥。
- **决策固化**:链目标先 **Sui testnet**;账号映射 `account_id == "sui:0x.."`(已存在,`gateway/src/auth.rs`)。
- **Exit**:`onchain/mir2-mine` 能 `sui move build` 通过;testnet 账号有 gas;CI 绿。

### M1 — 合约 MVP 上 testnet · 规模 M ·（L1）

- `dubhe.config.ts` 照搬设计 §3.1(data/schemas/events/errors/systems 全量)。
- `pnpm dubhe schemagen` 生成 Move;实现:
  - `mine_system::mine_batch`(§3.2:nonce 防重放 → 收 `Coin<SUI>` 入 treasury → `maybe_regen` → `sui::random` 按 hit_rate 25 / drop_rate 10 逐次 roll → `EPOCH_EMISSION_CAP` 封顶 → 落 `mine_state`/`ore_balance`/`miner_nonce` → emit `mine_settled`(+`mine_depleted`))
  - `redeem_system::redeem`(销毁链上矿石 → emit `ore_redeemed`)
  - `admin_system`(`init_mine` 写 `mine_config`;治理 setter 占位)
- **Move 单测**(`sui move test`):命中/枯竭/再生/排放封顶/重放/费用不足 各一例。
- 部署 testnet;记录 `packageId`、schema/对象 ID、部署 digest;写 `onchain/mir2-mine/README.md`(部署 + env)。
- **TS 烟测**(`@0xobelisk/sui-client`):`init_mine` → 一笔 `mine_batch` → 一笔 `redeem`,断言事件与状态。
- **Exit**:testnet 上有真实 `mine_batch` + `redeem` 成功 digest;`mine_settled`/`ore_redeemed` 事件字段正确;Move 测试全绿。

### M2 — 索引器 + Relayer 骨架(入站桥 dry-run) · 规模 M ·（L2+L3）

- **L2**:`dubhe.config.json` 让 `dubhe-indexer --config ... --network testnet --with-graphql` 起得来;本地验证能订阅到 `mine_settled`/`ore_redeemed`(GraphQL `ws`)。
- **L3**:`onchain/relayer/`(TS)——`createDubheGraphqlClient` 订阅事件 → 规整为入站意图(`GrantOnchainOre`/`CreditGoldFromOre`)→ **先 dry-run/结构化日志**,不连生产。内置:
  - **幂等**:`(tx_digest,event_seq)` 持久去重(SQLite/Redis);
  - **排序/最终性**:按 checkpoint 顺序消费,等足够确认数;
  - **重试/退避 + 死信**:下游失败重试,超限入 dead-letter + 告警;
  - **reorg 处理**:回滚未最终化事件的副作用(配合 M3 幂等键)。
- **Exit**:testnet 上一笔真实 `mine_settled` → 索引器 → Relayer 打印出**正确的入站意图**(账号=`sui:0x..`、ore_kind、amount、mine_id、stones_left),重复投递不产生重复意图。

### M3 — 服务端/Sim 入站集成(Rust) · 规模 M ·（L4）

- `WorldCommandKind`(`world_runtime.rs` 附近)新增并实现 + 单测:
  - `GrantOnchainOre { account, ore_kind, amount, mine_id, stones_left, idempotency_key }` → 写背包 + 广播 `MineNodeState{stage}`;
  - `CreditGoldFromOre { account, gold, idempotency_key }` → 加金币。
- **入站端点**:gateway/admin-api 加一个**鉴权**的 HTTP 端点(operator token,复用现有 admin 鉴权)供 Relayer 调用;带幂等键去重(双保险)。
- **对账**:乐观挖矿(P0)与链确认结果对齐——链确认为准,修正乐观偏差(数量/渲染),不重复发矿。
- **账号路由**:`account == "sui:0x.."` → 对应在线/离线角色背包(离线则入库待领)。
- **Exit**:Relayer(testnet 真事件)→ gateway → Sim 把矿石发到**正确账号背包**、广播渲染;重复事件不重复发矿;`cargo fmt --all --check` + `cargo test -p mir2-simulation` 全绿。

### M4 — 客户端端到端(挖矿→签名→确认) · 规模 L ·（L5）

- **发交易**:客户端攒满 N 挥 → 用登录钱包/passkey 签 `mine_batch`(`@mysten/sui`);失败/取消有回退。
- **乐观体验**:挖矿即播粉尘 + 临时矿石提示(已具雏形),链确认后由 M3 对账修正(乐观偏差回滚/补足)。
- **Redeem UI**:卖矿换金币入口 → 发 `redeem` → 经索引器/Relayer 回 `CreditGoldFromOre`;矿石/金币余额展示;`redeem` 进度与失败提示。
- **渲染分档**:消费 `MineNodeState{stage}` 切贴图(满/裂/空)——P0 广播已在,补客户端贴图三档。
- **Exit**:web 客户端真人:挖矿→签名→拿到链确认矿石→`redeem` 换金币→金币可用;矿点随挖随变贴图;testnet 全程跑通,录一段 e2e 证据。

### M5 — 经济与治理可调 · 规模 M ·（L6 + L1 admin）

- 合约 `admin_system` 暴露可治理参数 setter + 守卫(仅 admin):`fee_for_swings` 费率、批大小上限、`EPOCH_EMISSION_CAP`、`redeem` 兑率、再生周期。
- **金币 sink**:接 §5 经济——矿石→升级**可交易**装备 + **交易行抽成**(锁定决策③);赛季制温和抬顶(决策④);一级矿石成本略低于二级市场以保 sink。
- **Sponsored tx + 批量**:项目方代付 gas(或 fee→treasury 充当“水龙头价格上限”,§5.0);批内聚合降单笔成本。
- **算账校准**:用真实数把“挖 N 次成本 vs 矿石→金币产出”闭环跑一遍(§5.3 模板),定 `fee`/批大小/兑率初值。
- **Exit**:全经济闭环(挖→矿石→redeem→金币→升级装备/交易行)在 testnet 用**接近真实参数**跑通;参数可经 admin 治理热调;算账闭环为正且可持续。

### M6 — 硬化:负载 / 热点 / 最终性 / 安全(mainnet 前置闸) · 规模 L ·（L7 + L1 + L3）

- **Sui 热点对象**:把全局热写(`emitted_this_epoch`/`treasury`/热门矿 `mine_state`)分片/降争用(§6),压测高并发同矿。
- **最终性/reorg**:系统性测试 Relayer 在 checkpoint 回滚下不双发、不丢单;Sim 幂等键证明无重复入账。
- **安全审计**:合约(算术溢出、随机性、权限、重入语义)+ Relayer 信任边界(它能调发矿端点=高权,需最小权限 + 鉴权 + 限流 + 告警)。
- **风控/Admin**:异常矿石/兑换接入现有 audited command + 风控;kill-switch(暂停 `mine_batch`/`redeem`)。
- **监控看板**:treasury、矿石排放/留存、矿价曲线、Relayer 滞后/死信、合约调用量。
- **Exit**:审计问题清零或缓解;负载/热点达标;reorg 边界用例全过;看板 + kill-switch + 风控就位;runbook 写好。

### M7 — 服务器部署索引器/Relayer + testnet 灰度 · 规模 M ·（L7）

- 把 **Dubhe Indexer** + **Relayer** 做成 systemd 服务(参照 `infra/systemd/mir2-gateway.service` 的形态:`EnvironmentFile`、`Restart=always`、日志进 journal);env/密钥进 `/etc/mir2/*.env`(不入仓库)。
- 与 gateway 同机/同 VPC 部署;Relayer→gateway 入站端点走内网 + operator token。
- **testnet 灰度**:小规模真人灰度,盯 treasury/留存/矿价 + Relayer 健康,按 §5 调参。
- **Exit**:testnet 全链路在服务器常驻运行、被监控;一周 soak 无双发/丢单/资金错账;灰度数据支撑上主网 go/no-go。

### M8 — 主网上线 + 收尾 · 规模 M ·（L7 + 全体）

- **产品 go/no-go**(基于 §9 开放问题已拍板 + M7 灰度数据)。
- 合约发主网(新 `packageId`),Indexer/Relayer 指向 mainnet;分批灰度放量 + kill-switch + 回滚预案。
- 复盘审计整改闭环;文档收口(运维 runbook、经济参数台账、应急预案);把 `ONCHAIN-SMART-MINE-DESIGN.md` 状态从“草案”更新为“已上线”。
- **Exit(= 完工)**:第 0 节 Definition of Done 全满足。

## 4. 关键路径与并行

- **关键路径(串行)**:M0 → M1(合约) → M3(Sim 入站) → M4(客户端 e2e) → M6(硬化) → M7(部署灰度) → M8(主网)。
- **可并行**:
  - M2(索引器/Relayer)可在 M1 完成后与 M3 **并行**(M2 dry-run 不依赖 M3,但 M3 联调需要 M2)。
  - M5(经济/治理)可在 M3 起与 M4 **并行**起草,M4/M5 在“闭环联调”处汇合。
  - L7 看板/CI 贯穿全程,尽早起。
- **汇合点**:M4 需要 M2+M3 都通(真事件能落到背包);M8 需要 M5(经济)+M6(安全)+M7(灰度)全绿。

```
M0 ─ M1 ─┬─ M3 ─ M4 ─┬─ M6 ─ M7 ─ M8
         └─ M2 ───────┘
              M5 (并行起草, 在 M4/闭环汇合)
```

## 5. 跨切面(贯穿所有里程碑)

- **测试金字塔**:Move 单测(L1)→ TS 烟测/集成(L1/L3)→ Rust 单测(L4)→ 端到端(testnet 真链 e2e,M4/M7)。每个 PR 各层测试必绿。
- **CI**:`onchain/` 加 Move build+test、TS lint/build 门禁;现有 `cargo fmt`/`cargo test`/web `tsc` 门禁不破。
- **密钥/机密**:私钥/助记词/operator token 一律进 env / secret store,**绝不入仓库**;Relayer 与 gateway 间走 token 鉴权。
- **幂等/对账**:三处幂等(链 nonce、Relayer digest 去重、Sim 入站幂等键),任意一处兜底防双发。
- **可观测**:结构化日志 + 指标(treasury、排放、Relayer 滞后/死信、矿价、失败率);kill-switch 贯穿合约/Relayer/Sim。
- **回滚**:每层独立回退——合约(暂停系统)、Relayer(停消费)、Sim(停入站端点)、客户端(隐藏入口、降级为纯 P0 服务端挖矿)。

## 6. 上线前必拍板的开放决策(§9,给推荐默认值)

| 决策 | 推荐默认(可改) | 影响里程碑 |
|---|---|---|
| 金币是否外部回兑 | **否,纯付费货币**(锁定决策②) | M5/M8 合规 |
| 矿:可再生 vs 有限枯竭 | **可再生 + epoch 排放封顶**(决策 C) | M1/M5 |
| `fee_for_swings` / 批大小 / 兑率初值 | M5 用 §5.3 算账模板代真实数定;先给保守占位 | M5 |
| 链目标 | **Sui testnet 全程 → mainnet 上线**;Movement 留后续 | M7/M8 |
| 首矿选址 | 起始图 `0` 合成安全矿先可玩(A),DeadMine 入口并行排期(B) | M1/M4 |

> 这些不阻塞 M0–M4 的工程实现;M5 经济联调前必须把费率/兑率/排放拍死,M8 主网前把链目标与合规拍死。

## 7. 风险登记(Top)

| 风险 | 缓解 |
|---|---|
| Sui 热点对象并发(全局 treasury/排放/热门矿) | M6 分片/降争用 + 压测;批量结算少写 |
| 链延迟/reorg 导致双发或丢矿 | 三处幂等 + 最终性等待 + reorg 回滚 + 对账 |
| Relayer 高权(能调发矿端点)被滥用 | 最小权限 + 鉴权 + 限流 + 审计 + kill-switch |
| 经济不可持续(矿石无 sink → 金币贬值) | 矿石→升级装备/交易行抽成 sink + 排放封顶 + 赛季抬顶 |
| 合约漏洞(随机/算术/权限) | Move 单测 + M6 审计 + 主网前闸 |
| 工具链/Dubhe API 漂移 | 以 `0xobelisk/Dubhe` 仓库 examples 为准,锁版本 |

## 8. 一页纸排序(给执行者)

1. **M0** 工具链 + `onchain/` 工作区(S)
2. **M1** 合约 MVP 上 testnet(M)
3. **M2 ∥ M3** 索引器/Relayer dry-run ∥ Sim 入站接口(M+M)
4. **M4** 客户端 e2e(挖→签→确认→redeem)(L) ·（M5 经济并行起草)
5. **M5** 经济/治理可调 + 闭环算账(M)
6. **M6** 负载/热点/最终性/审计(L)
7. **M7** 索引器/Relayer 上服务器 + testnet 灰度(M)
8. **M8** 主网上线 + 收尾(M)= **完工**
