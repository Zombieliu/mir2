# 个人智能求购第一版

状态：2026-10-29，V1 代码已实现。**Windows 真实测试账号 → Gateway → File 游戏资产权威 → Dubhe 测试网 → 游戏到账的发布成交、取消退款、120 秒到期退款三条实时流程已通过自动化验收；18 份快照和 9 笔真实交易的独立核验通过，双方最终结清后再次重登也通过。** 原生服务器 PostgreSQL、生产收费、多机长期恢复及人类前端 Accepted 仍未通过。本文不表示生产就绪。

本文件是个人求购的主题汇总，原始日志集中列在“验收与证据状态”，不把 Mock、WASM SQL、真实测试网夹具和 Windows 实连合并成一次验收。

## 本轮代码基线与范围

- 实际 checkout：由仓库根路径解析 `mir2-web3`，本轮使用已附着的 `cell-preproduction-p0-p1` 工作树。
- 基线提交：`2a1c54c29b9a12a6e0f381be1c13168ecbc49f8f`。
- 专用分支：`codex/personal-procurement-v1-20261028`。
- 开始时没有 tracked 修改；四个既有 capacity 证据目录保持未跟踪，不纳入本轮。
- 仅个人求购；全服同服玩家交货，收货人为个人。未来以收货目标扩展点接行会，不开发行会代币、沙巴克仓库、组织治理或其他经济调参。
- 不启动或修改 Docker，不部署主网，不使用真实资金，不开启生产收费，不生成生产密钥。已完成的部署仅为隔离测试网合约与测试币验证。

## 已确定规则

1. 第一版只有单件装备、数量 1、一次完整成交，无部分成交、竞价或跨服。
2. 示例为 500,000 游戏金币求购井中月。运行条件使用 **template 245 / `crystal-item-245` / DragonSword**；中文只作展示。
3. 金币、点券和完整装备实例仍在游戏服务器数据库，无金币 Token、装备 NFT 或 USDC 货款。
4. B 路径：服务器先事务锁定资产，链上裁决发布、成交、取消和到期，服务器只依据已核实的最终结果交付。
5. 买家金币托管和服务额度预留必须与持久待发送任务同事务。初始化链上订单确认后才公开、确认消费发布额度。
6. 交货锁定真实 UID 的原始装备实例，保留耐久、强化、附加属性和孔位；不调用模板生成器替代交付。
7. 最终结算先产生持久到账资产；在线角色在串行 owner checkpoint 内消费到账资产并更新钱包/背包。离线或背包满保持待领取，不能写旧在线快照覆盖新资产。
8. 链上已成交与游戏内已到账分别展示。服务器是可信身份核验者和资产保管者；合约不能验证数据库实物，也不能强迫离线服务器交货。
9. 智能求购服务费独立于基础月卡；一次发布费覆盖该订单后续成交、取消、到期与技术重试。卖家无需另购订阅，首版不叠加金币成交税。
10. 新发布关闭或额度过期不能阻断旧订单对账、交付、退款和领取。未知结果不因网络超时释放资产。

## 已核查可复用代码和缺口

| 能力 | 实际代码 | 本轮处理 |
| --- | --- | --- |
| 多账号原子持久化 | `apps/simulation/src/config.rs` 的 `commit_account_store_transaction`；File 原子替换、Postgres 单事务及版本 CAS | 复用账号级 JSON ledger，不新增全局可变订单热行；扩展采购未知提交冻结与 schema 守卫 |
| 在线角色一致性 | `runtime/npc_purchase_transaction.rs`、`runtime/billing.rs` 的 checkpoint/revision/pending 应用 | 复用串行 owner 流程，不能仅扣 sidecar 账本或让后台覆盖在线 save |
| 完整物品托管 | `runtime/items.rs`、`item_custody.rs`、`social_economy.rs` | 复用完整实例和 UID 校验，补真实背包、绑定、装备、交易/租赁及其他托管限制 |
| 原始物品 | `packages/game-data/data/generated/crystal_item_manifest.json`，`platinum_176.json`；`NUMERON-ITEM-CHINESE-NAMES-20261001.zh-CN.md` | 245 为 DragonSword，34000 耐久、单件、基础攻击 7–22；241 为凝霜，不采用名称猜测 |
| 金币 | `CharacterSaveRecord.gold: u32` | 上限 4,294,967,295；全部 checked 运算，超限保留待到账资产 |
| 充值账本 | `billing.rs`、`monthly_card.rs`、`gateway/stripe_billing.rs` | 复用支付证据与幂等模式；比例读取真实 offer 配置，不假设美元兑换率 |
| 点券来源 | 当前可用 `credit` 为混合余额 | 不能将混合余额当现金收入；采购额度单列 Test/Paid/Gift/Unknown 批次，生产购买默认关闭 |
| NPC 入口 | `runtime/npc.rs`、`npc_script.rs`、`stone_npc.rs` | 复用真实菜单、距离、存活和当前会话校验；新增独立求购面板 |
| Windows 服务流程 | `native_billing.rs`、`native_protocol.rs`、`shell_bridge.rs` | 复用账号/角色关联、未确认状态及稳定请求 ID，独立求购协议和 UI |
| 账号钱包绑定 | `gateway/identity.rs`、`web.rs` | 只用于身份关联，不当作任意交易授权；每次明确确认独立签署订单意图 |
| 旧链上桥 | `runtime/onchain.rs`、`onchain/relayer` | session HashSet、先去重后 sink 与分文件游标不满足交付要求；新增持久 inbox/outbox 和主动最终状态核验 |
| 旧 Dubhe 包 | `onchain/package.json`、`mir2_mine/Move.toml` | 真实为 1.2.0-pre.124 矿场；本轮独立包，不升级或挪用旧矿场经济模型 |

本轮新增和集成的主要文件：

| 范围 | 实现文件 | 已实现边界 |
| --- | --- | --- |
| 服务端订单领域 | `apps/simulation/src/procurement.rs`、`procurement/tests.rs`、`runtime/procurement.rs` | 账号 ledger、真实金币/完整装备托管、额度、持久状态机、outbox/inbox、原实例待领取、幂等与主动恢复 |
| 持久化保护 | `config.rs`、`billing.rs`、`db_projection.rs`；`infra/postgres/migrations/0018_procurement_authority_guard.sql` | schema 12、校验、删除/降级保护、未知提交冻结，含测试套餐一次领取回执；未迁移任何生产数据库 |
| Gateway | `apps/gateway/src/procurement_worker.rs`、`web_procurement.rs`、`routing/procurement_owner.rs` | 已认证角色的串行 owner/租约路由、持久 exact transaction、轮转调度、实时核验与游标恢复 |
| 共享协议与 Windows | `packages/protocol/src/procurement.rs`；`apps/game-client/client-bevy/src/native_procurement.rs`；Windows `native_protocol.rs`、`shell_bridge.rs` | NPC 入口、公开列表与模板筛选、发布/交货明确确认、我的订单、服务明细、待领取区；三条真实 Windows 实连流程及限制见下文 |
| 价格与来源分类 | `apps/simulation/src/procurement_fees.rs` | 基于真实充值 offer 的整数报价与来源分类纯函数；不扣点券、不调用支付平台、不发行付费额度 |
| 合约和链驱动 | `onchain/procurement` 的 Move 系统、`dubhe.config.ts`、`service/canonical.mjs`、`driver.mjs`、`grpc.mjs` | 真正复用 Dubhe schema/ObjectStorage/SDK；独立订单对象，GraphQL 提醒，gRPC 核验最终资产结果 |

发布、交货和领取使用当前在线角色的 checkpoint/revision，资产变化与 ledger/outbox 在同一持久事务中提交，成功后才更新 ECS。后台结算仅写待到账资产与回执，不覆盖在线角色的 save。金币、全部托管和未领取金币合计守恒；同一实例 UID 的存量托管与可用背包必须互斥。

## Dubhe 接入决策

上游审计固定 `0xobelisk/dubhe` commit `04700a9a13e6f279be3709e84bf4ad24b4349af3`。框架依赖的 Sui 固定 commit `3c0f387ebb40b8be292d3b7bd3f5bee8ad226d33`（mainnet-v1.68.1 **编译器/框架版本名称**，不表示部署主网）。新包位于 `onchain/procurement`，与旧矿场依赖隔离。

实际依赖固定为 `@0xobelisk/sui-client@2.0.0`、`@0xobelisk/sui-common@2.0.0` 和该 SDK 的 `@mysten/sui@1.35.0`。网络传输单独使用 exact alias `@mysten/sui-modern = @mysten/sui@2.16.2`；不使用旧 SDK 的 JSON-RPC 传输。锁文件与 `UPSTREAM.json` 记录了源码和依赖边界。

真实接入点包括 `dapp_system::create_dapp`、`create_and_share_typed_object`、`dapp_service::ObjectStorage<Order>`、`set_object_field`、`Dubhe_Object_Created`/`Dubhe_Object_SetField`、DApp guard，以及 Dubhe 的 PTB 构造与已构造交易离线签名。`dubhe.config.ts` 描述实际字段，业务状态机由手写 Move 系统实现；不宣称单靠 schema/codegen 就完成了托管。

标准 ObjectStorage 构造先创建空对象并登记唯一 entity，再初始化条件，因此发布实测为 **create + activate 两笔交易**。空对象不公开、不消费预留额度；只有初始化确认后成为 Open。每单数据独立，所有订单复用同一合约。创建时登记 DappStorage；后续成交、取消、到期、撤销只修改对应订单，对 DappStorage/Config 作只读校验，避免全部成交争用一个全局可变对象。

玩家每次明确确认短期用途公钥的 canonical BCS 意图，签名对象为 `SHA256(BCS(IntentV1))` 的 raw Ed25519 消息。服务器另签 OracleProof，绑定网络、合约、DApp、订单/成交 ID、双方身份、条件、真实资产锁、有效期和 fence。取消证明引用原发布金币锁；授权期限不晚于订单到期。链上保存条件 BCS 原文和哈希，不能只凭中文名称或登录钱包关联认定交易授权。

默认事件补拉改为**官方过滤 GraphQL**：按本包 `procurement_system` 和部署 checkpoint 查询，持久化 opaque edge cursor，包括空页进度。实测两页恢复 9 笔订单交易的相关事件，首事件 cursor 重启无重复。GraphQL、普通 Dubhe indexer 和事件订阅只提供提醒/展示，**唯一资产终态核验路径是可信一致 fullnode 的 gRPC 交易 effects、checkpoint、BCS 对象/事件和新鲜订单读取**。每单主动对账独立于索引是否在线；显式 checkpoint 扫描仅作小型 localnet/专用节点的后备，不把公共全链扫描当默认监听。

补偿观察必须先读取已确认链上 Clock，再读取完整订单字段，重读订单版本/previous transaction，并绑定最终 effects/事件；注册表缺失使用单独的稳定双读证据。不能把旧状态配上之后的 Clock 来退款，也不能把索引中的“成交”直接当到账依据。当前方案信任一个一致 fullnode/粘性端点，不是轻客户端密码学快照证明。

不采用 Marketplace：其是链上 `Coin<CoinType>` 卖单，默认 3% 费率；pinned 源码中费率与 DApp 分成更新均检查 framework admin。它不是数据库金币买单接口，不能沿用其成交费。

上游 SDK 默认 network 为 mainnet，缺少 key 时会自动生成 mnemonic。采购适配器必须在实例化前显式验证 network、chain ID、package、framework、DApp 和 signer，disabled 模式不得实例化。只接受白名单采购调用，Gas 预算与总额限额独立于玩家服务费。

权限分开：本包 `AdminCap` 控制新发布开关和 Oracle 换钥并发出事件；Oracle 核验密钥只能为经认证确认且已持久锁定的意图出具证明；Gas sponsor 仅签白名单采购调用；包升级仍受 Sui `UpgradeCap` 权限控制。以上权限均不提供任意改游戏金币/背包的接口。生产密钥保管、管理员/升级治理和换钥恢复演练仍需单独批准与验证，不能仅改配置就释放旧证明对应资产。

参考：[Dubhe 源码](https://github.com/0xobelisk/dubhe/tree/04700a9a13e6f279be3709e84bf4ad24b4349af3)、[官方文档](https://dubhe-docs.obelisk.build)。源码与文档不一致时以锁定源码和测试为准。

## 实施顺序与剩余门槛

1. 已实现：共享签名协议和持久 ledger，包含规则快照、schema/删除/未知提交守卫。
2. 已实现并有真实隔离 File 事务测试：owner 金币/原实例托管和领取，同事务预留额度/outbox，持久提交后发布内存。
3. 已实现并通过独立真实测试网合约流程：两阶段发布、互斥终态、证明 fence、exact bytes/digest、断线补拉和主动对账。早期托管夹具和随后真实游戏 File 权威实连有独立证据，不混算为一次测试。
4. Windows NPC 与五类页面、明确确认和共享协议已实现；实际测试角色/游戏资产/测试网的成交领取、取消退款、到期退款三条实时流程通过，含 Open 订单跨服务重启保留与买家离线待领。
5. 剩余验收门槛：原生服务器 PostgreSQL/Rust 驱动、多 Gateway 并发与长期恢复演练、生产运维、真实付费来源接入和人类前端 Accepted。未通过生产门槛前不开放生产。

## 收费：试行建议，尚未批准正式价格

- 用户建议约 0.10 美元/次、10 次约 1 美元，均不作为生产价或盈利结论。
- 套餐点券价格必须由实际充值 offer 的 currency/amount_minor/credits 和明确舍入规则计算；无可靠比例时禁售，不用猜测默认值。
- 成功发布后扣一次额度；明确发布失败且已证明不会迟到生效才释放预留。技术重试不得再次计费。
- 建议主动取消/到期未成交不退已完成发布费；退款规则、费率、套餐、有效期和开放条件按版本配置并随单冻结。
- 测试使用显式测试批次；赠送与未知来源不计现金收入，不能据混合点券余额宣称付费来源。
- 官方独立 Gas 预算赞助白名单操作。点券是服务收费凭证，不直接作为链上 Gas。

已实现的报价纯函数按明确选择的 `RechargeOffer` 计算：`ceil(套餐货币最小单位金额 × offer.credits / offer.amount_minor)`，整包只向上取整一次，校验币种、offer 唯一性、整数溢出、版本批准与有效期，并冻结引用 offer。它没有固定“1 美元兑换多少点券”的假设，也没有执行扣款或授予额度。

**生产购买仍关闭，来源接入尚未完成。** 现有点券余额没有逐批已付费/赠送消费来源，不能通过加一个 Paid 标签解决。玩家 `QuoteBuyQuota` 只支持下述显式免费 Test 套餐，所有真实付费套餐仍拒绝。纯来源分类函数只有在已有认证充值账本中的 `Applied`、有效支付引用、`livemode` 和额度应用证据一致时才认作 Paid；测试支付仍归 Test。该校验尚未接入一条可生产扣券、追踪来源和发放套餐的事务流程，不能据此宣称生产收费能力已上线。消费预付点券也不再记一次充值现金收入。

待确认的正式配置包括：批准的规则版本、套餐次数、币种与报价、参考充值 offer、可用额度来源、有效期、取消/到期退款规则和开放环境。默认 `production_charging_enabled=false`；试行建议不等于已批准价格。

### 已实现的免费 Test 套餐与完整规则配置

同时开启新发布和显式 `MIR2_PROCUREMENT_TEST_CREDITS=1` 的隔离环境，在 NPC 的服务页显示 `test-free-10-v<规则版本>`。这是**免费测试额度，非付费套餐、非现金收入**：每账号、每规则版本最多领取一次，相当于 10 次该版本发布的额度（`10 × fee_units`）。生产/预发布环境的 Test 开关被拒绝，`production_charging_enabled=true` 同样不能启用此免费通道。

玩家先请求报价，再在同一已认证角色的有效 NPC 上明确确认。测试报价没有链上意图，不要求伪造一份链上交易签名；确认只发行 source=Test 的额度批次，不扣金币/点券、不发链上交易。报价、冻结规则、幂等请求和一次领取回执持久保存。并发确认、超时重试、进程重启或换一个报价 ID 都不会重复授予同版本套餐；已完成请求在功能关闭/报价过期后仍可返回重放结果。未确认的过期报价不授予额度。

`MIR2_PROCUREMENT_RULE_JSON` 接收完整、禁止未知字段的 JSON，例如以下默认规则（示例配置，不执行环境变更）：

```json
{
  "version": 1,
  "template_id": 245,
  "fee_units": 1,
  "cancel_fee_refundable": false,
  "expiry_fee_refundable": false,
  "maximum_lifetime_ms": 604800000,
  "maximum_authorization_ms": 300000
}
```

所有字段必须提供；版本须大于 0，模板固定 245，`fee_units` 为 1–1000，订单最大寿命为 1 秒至 30 天，授权最长为 1 秒至 15 分钟且不超过最大寿命。超过 4096 字节、缺字段、未知字段、错误类型、零值/越界以及与同时设置的 `MIR2_PROCUREMENT_RULE_VERSION` 不一致都会报错，不能静默回退。新意图的实际授权期限还会取订单到期的较小值。

变更任何费率、退款或期限必须提升版本；已有订单和 Test 套餐报价保存完整规则快照。同一账号已保存相同版本却内容不同的订单/套餐报价时，新操作拒绝版本复用。该机制没有增加全服结算热对象；多进程部署须使用一致、不可原地覆写的规则配置。旧单取消/到期/领取继续按其冻结规则执行。

## 真实测试网交易数与费用样本

2026-10-28 的独立合约实测使用隔离开发密钥和官方 faucet 测试币：同一合约下三张独立订单，**9 笔订单交易和 3 笔一次性设置交易全部确认成功**。这一批标记为 `custodyFixtureOnly=true`、`realDatabaseDelivery=false`。随后 Windows/Gateway 真实 File 权威联调又产生三张独立订单、9 笔确认交易，`custodyFixtureOnly=false`；其实际每条流程 Gas 恰与下表一致，逐笔回执另存实连证据，不重复使用早期夹具证明游戏交付。

| 路径 | 实测交易数 | 净 Gas（MIST） | 净测试 SUI |
| --- | ---: | ---: | ---: |
| 发布：create + activate | 2 | 33,099,970 | 0.033099970 |
| 发布 → 成交 | 3 | 34,100,931 | 0.034100931 |
| 发布 → 取消 | 3 | 34,100,418 | 0.034100418 |
| 发布 → 到期 | 3 | 34,100,418 | 0.034100418 |
| 一次性部署 package | 1 | 70,600,800 | 0.070600800 |
| 一次性 Dubhe genesis | 1 | 10,029,400 | 0.010029400 |
| 一次性启用发布 | 1 | 1,000,358 | 0.001000358 |

净 Gas 公式为 `computationCost + storageCost - storageRebate`；回执中的 `nonRefundableStorageFee` 单独披露，不重复加一次。发布两步分别为 6,442,063 和 26,657,907 MIST；成交增加 1,000,961，取消/到期各增加 1,000,448。三条生命周期与设置合计实耗 **183,932,325 MIST（0.183932325 测试 SUI）**。额外撤销、已确认失败后的新尝试和后续存储策略会增加成本。

Windows 真实三单净 Gas 合计另为 **102,301,767 MIST（0.102301767 测试 SUI）**，每单发布均 2 笔 33,099,970 MIST，再分别加一次成交、取消或到期；不含一次性设置、早期夹具、RPC/索引/服务器成本。上述是测试网当时参数和最小属性条件的样本，不是生产 Gas 报价。测试币没有真实现金价值；没有换算美元，也没有证明 0.10 美元/单可以盈利。

| 非 Gas 成本 | 当前测量状态 |
| --- | --- |
| 游戏服务器 CPU、内存、带宽和可靠 worker 容量 | 未测量本功能增量及生产预算 |
| 数据库托管、历史存储、备份与恢复 | 未测量 |
| gRPC/RPC 请求、流量和服务额度 | 未计费测量 |
| 过滤 GraphQL/自营索引的计算、数据库与保留期 | 未计费测量；公共测试网端点不代表生产 SLA |
| 密钥托管、监控、故障处理与运维 | 未测量 |

## 验收与证据状态

| 验证层 | 已取得证据 | 不能据此认定的范围 |
| --- | --- | --- |
| Rust 领域、真实 File 与 owner | 最终 `cargo +1.95.0 test --locked --offline -p mir2-simulation --lib procurement`，54/54；含报价/来源分类、Test 套餐完整 owner 请求、并发领取、schema-12、报价冻结、120 秒租约/慢 RPC、稳定 wire 状态和原实例描述。另有相邻 schema 回归 3/3 | 该组链上最终回执为 Mock；未替代真实 PostgreSQL，Windows 实连另有独立记录 |
| Gateway 可靠 worker | 最终资产投影集成 v6 测试 13/13；含隔离 File + Mock 驱动恢复、exact bytes、失败归档、游标和原实例交付。耗时公平性一项沿用此前 v4 14/14 快照，不重复累加；最终 v5 构建成功 | 这组测试本身不代替网络和 GPU 客户端实连 |
| Node 合约驱动 | 21/21，包含 BCS/签名、域/白名单、Clock 一致性、GraphQL 游标和有界 checkpoint 后备 | 单元测试不代替真实网络或数据库 |
| Move 合约 | 14/14，build 与 warnings-as-errors 通过；仅 genesis 同事务新对象 share 的已说明 lint 抑制 | 未声称原始 Git-pinned manifest 在当前离线网络直接构建成功 |
| 真实 Sui 测试网 | 3 条订单生命周期，9 笔订单 + 3 笔设置交易；最终对象/effects/事件与实际 Gas 已核验 | 托管夹具；无游戏数据库交付证明 |
| 真实 GraphQL 补拉 | 官方过滤查询 2 页、9 个相关事件，12.304 秒；从首事件 opaque cursor 恢复不重复 | 仅提醒/游标证据；不能裁决资产，不能推导生产吞吐和 SLA |
| PostgreSQL SQL | 官方 PGlite 0.5.13，PostgreSQL 18.6 WASM，44/44（含 schema-12 Test 套餐保护） | 内嵌内存库上的 SQL/PLpgSQL，不是原生 PG 服务、Rust 驱动、多进程或崩溃持久性验收 |
| Windows 模型与 GPU | 最终 v3 native 逻辑 13/13，另 1 GPU 测试 ignored；此前 Windows host wire/bridge 3/3、协议 1/1。历史五页加两确认共 7 图，后续短期到期发布/确认另 2 图，missingGlyph/overflow/outside 均为 0；v3 没有重跑 GPU | 离线夹具明确 `serverConnected=false`、`testnetTx=0`、`humanAccepted=false`；截图有各自源码快照，不冒充最终 v3 重渲染或实时实连 |
| Windows 实时 E2E | **三条通过**：真实 File 游戏资产发布→成交→双方领取、发布→取消→退款领取、120 秒发布→真实 Expired→退款领取；独立核验含双方最终重登的 18 快照、9 笔真实交易通过 | 隔离测试账号、测试币和自动化 Windows 操作；不是人类 Accepted、原生 PG、多机长期恢复或生产收费验收 |

领域测试覆盖正常成交、金币/额度不足、错误模板/绑定/已装备/不满足条件、重复请求、并发交货、成交/取消互斥、到期、伪造/跨环境重放、完整实例与金币守恒、最多一次服务消费、背包满/金币上限、离线待领取、未知提交重启、链上成功但 File 提交失败、短期授权、已确认失败尝试归档，以及取消证明过期/更高 fence 后重新确认。覆盖是这些测试的范围，不能扩张为所有真实部署故障都已验收。

PGlite 在临时 `accounts(account_id, raw_json JSONB)` 表上执行仓库原始 0018 三次，验证幂等；普通及旧 `procurement:null` 账号可更新/删除；有 authority 的账号不能删除、置 null 或省略 ledger；10 类历史集合不能删除、清空或替换掉既有键，包含 `quota_quotes`、`quota_purchases`，并专测旧 schema-11 writer 遗漏新回执被拒绝；正常状态推进、追加历史、失败交易归档仍允许；触发器拒绝和应用异常均回滚事务，连同一次领取回执的写入一并回滚。未连接数据库 URL，未启动 daemon 或 Docker。当前测试 SQL 的 SHA256 为 `53b63e26318a345c9fc7dc571e6f6a50c9b4534829130a2ea4519446a7f03ac4`。

本主题原始证据入口：

- [链上主题证据](../onchain/procurement/validation/20261028/README.md)：结果、逐笔回执、Gas、21 项 Node 日志、14 项 Move 结果、官方 GraphQL 补拉、编译器与源码等价记录均由此进入。
- [公开证据总入口](generated/personal-procurement-v1-20261028/README.md)：相对路径、原始 artifact SHA256 与明确按 CRLF→LF 规范化的源码清单。
- [Rust 最终 54 项](generated/personal-procurement-v1-20261028/rust/procurement-tests-v17-stable-wire-states.log)、[schema 3 项](generated/personal-procurement-v1-20261028/rust/procurement-schema-regression-v3.log)、[Gateway 最终 13 项](generated/personal-procurement-v1-20261028/gateway/gateway-tests-v6-asset-projection.log)。历史快照单独保留，不累计成新覆盖。
- [PGlite 44 项](generated/personal-procurement-v1-20261028/sql/results.json)及同目录原始 SQL、脚本、exact npm 锁文件和完整性记录；较早 37 项只作历史用途。
- [Windows v3 逻辑与构建](generated/personal-procurement-v1-20261028/client/v3-fixture/native-focused.log)、[历史 GPU 清单](generated/personal-procurement-v1-20261028/client/initial-fixture/manifest.json)、[后续 2 张 GPU 图](generated/personal-procurement-v1-20261028/client/final-fixture/gpu-short-expiry/procurement-native-gpu.json)；这些与实连证据分目录保存。
- [Windows 实连及独立三单核验](generated/personal-procurement-v1-20261028/windows-live/README.md)：只含公开投影快照、真实回执、JPEG 及 sidecar，不含账户整库、密码、私钥或环境文件。

### Windows 实时验收：三单闭环通过

2026-10-28 至 29 日（Asia/Shanghai），以 Windows 原生客户端的真实已认证测试角色 `buyqa1028` / `sellqa1028` 操作 NPC 和明确确认界面。Gateway 使用同进程唯一 File 游戏资产权威和独立测试网 driver，没有把侧账本扣款或单独链脚本当作游戏交付。最终 Gateway v5 SHA256 为 `4b5bc8a10793583e0631ab9145862d86ad209c8a58f1481f1875f6892569060c`；Windows v3 为 `1a8c941bac3ae9be03bf64652dc64b555d8699a6a65bd65c9ca532da6b0bc144`。原始 process 凭据、逐笔 digest/checkpoint 与窗口侧录由实连目录关联。

| 真实路径 | 唯一订单 ID | 实际结果 | 交易数 / 净 Gas |
| --- | --- | --- | --- |
| 发布→成交 | `0x70cdaa52f85d1e1e787fa40749cf5f3ad72245a6399fdfdc24caa8a30807a14f` | Filled；买家离线时原件进入持久待领，随后原 UID 领取入普通背包；卖家领取 500,000 金币 | 3 / 34,100,931 MIST |
| 发布→取消 | `0x243e18275fd667bc4d29082c906bfe963b021f9b942a47bde3a402b427452d11` | Cancelled；明确确认取消，未决期间保持托管，最终 500,000 金币退款已领取 | 3 / 34,100,418 MIST |
| 120 秒发布→到期 | `0xcbe276110159616beafbe56a648b2e117518e04bc7c6ead04ec5aa440421de49` | 先成功 activate，再真实 Expired；并非 PublishFailed 补偿，500,000 金币退款已领取 | 3 / 34,100,418 MIST |

[独立审计汇总](generated/personal-procurement-v1-20261028/windows-live/evidence/live-three-scenarios-independent-audit.json) 的 `passed=true`：18 份阶段快照均验证金币守恒、UID 唯一有效归属和完整装备字段；三单链上终态、游戏终态和全部领取回执一致。总金币始终为 **2,011,280**，其中买家 **1,500,000**、卖家 **510,000**，其余原有账号金币 **1,280** 不变；最终托管 **0**、待到账金币 **0**。显式隔离 Test 预置 10 加 NPC 免费 Test 套餐 10，共授予 **20**，三次成功发布各消费 1，最终消费 **3**、预留 **0**、剩余 **17**；没有生产付费、金币成交税或重复链步骤收费。

原件为 template 245、UID **1028024501**，耐久 **31400/34000**、强化 `refined_value=1` / `refine_added=2`、MinDC +2 / MaxDC +3 均完整保留。原始锁定 JSON 的 SHA256 `7bfb7db030cfac6479a8fda3e18d8bc6bd2452a2b060d7d1d64aec5b01c63579` 与真实链上 `asset_instance_hash` 一致；全原件字段的规范化 SHA256 为 `9132a6c0cf9e76df2653996d435ae2cd8765f60460d04a76de29a686e1365b3d`，领取后字段全部一致。历史 released lock / claimed receipt 保留审计副本，排除后有效拥有者只有买家背包一处，不能把历史副本误算为复制装备。

本轮真实联调发现并修复四处连接问题，修复后保留原订单与托管，没有重新发布或再次扣费：

1. NPC 原菜单四行正文加五项选项已占首屏，求购追加项落到第二页；改为首项插入，真实 snapshot/九行分页回归通过。
2. 报价来源曾是解释长句，客户端要求精确 `test`；统一 machine-readable 来源，客户端按完整 status 费率规则比对，动态费用单位和冻结规则保留。
3. 域状态 Debug 输出 `Open`，客户端按钮要求 `open`；改为显式稳定 `pendingPublish/publishFailed/open/filled/cancelled/expired`，到账/退款状态依据真实 receipt 投影。
4. 采购单独发 `ServerPacket` 时正常 HUD/背包仍旧；成功且非 unknown 的响应补发权威 `worldSnapshot`，普通钱包、背包与原件 tooltip 已在 Windows 查看。

Open 订单跨 Gateway 重启后仍保留，同一订单恢复后才交货；买家离线待领、双方分别领取及取消/到期退款已实测。**双方分别领取不等于再次发送同一个领取请求的真实重放验收**；重复请求/并发等故障覆盖仍按 Rust/Mock 证据表述。双方最终结清后再次真实重登通过：[买家普通背包](generated/personal-procurement-v1-20261028/windows-live/screenshots/25-final-buyer-relogin-inventory.jpg) 保留原剑与 1,500,000 金币，[卖家普通背包](generated/personal-procurement-v1-20261028/windows-live/screenshots/26-final-seller-relogin-inventory.jpg) 已无原剑、金币 510,000；[第 18 份最终快照](generated/personal-procurement-v1-20261028/windows-live/evidence/final-after-both-relogin.json) 再次确认总金币 2,011,280、托管/待到账均 0、UID 仅归买家、三单终态与领取回执及额度消费 3 不变。真实窗口画面是电脑控制保存的 **JPEG**，sidecar 记录窗口、时间，另有阶段快照关联；不是 native capture PNG，也不是 Web 截图。此为自定义功能的自动化验收，不代替人类 Accepted。

## 迁移、回退和恢复

### 已实现迁移与安全回退

1. `AccountRecord.procurement` 为可选 ledger，当前账号持久 schema 为 **12**。最初订单领域使用 11；新增 Test 套餐报价/领取回执后升 12，防旧 11 reader 忽略新字段再覆写。仍复用 `accounts.raw_json` 和原有事务/CAS。必须仅配置一个 durable source：隔离 File 或 Postgres SourceOfTruth；不允许以双写副本作为结算权威。
2. `0018_procurement_authority_guard.sql` 已注册到迁移列表。它增加 PL/pgSQL 函数与 `accounts` 的 UPDATE/DELETE trigger，不重算游戏经济余额。存在 procurement 后禁止删除账号、丢弃/null ledger，以及删掉 `orders/locks/credits/outbox/deliveries/chain_inbox/absence_inbox/requests/quota_quotes/quota_purchases` 既有历史键；正常状态更新和追加仍允许。
3. 未携带新套餐字段/规则快照的合法 schema-11 存档前向迁移到 12；旧 `StoredQuote` 保持可读取，新增 `rule: Option<Rule>` 默认 None，两个新集合默认空值。新的发布报价保存完整规则；旧版未确认发布报价缺少快照时必须重新报价，不能以当前费率静默确认。错误标记为 11 却已经携带新套餐 authority 或新报价规则快照的存档拒绝自动提升。新 File authority 同时保留 billing ledger，旧 schema-10/11 的未来 schema 拒绝条件及回归已核对；未宣称运行旧二进制完成 downgrade 演练。File 需要保留新版本恢复程序，不能用旧程序覆写新文件。
4. 正式 PG 环境实施前须保留可恢复备份、停止旧 writer，并在独立环境验证该环境的完整迁移和 Rust 驱动事务，再使用 schema-12 程序。0018 尚未在生产应用；本轮候选内的 SQL 扩展仍须纳入首次正式迁移。当前仅取得 PGlite SQL 证据，没有修改生产数据库。
5. 回退关闭**新增发布和套餐购买**，保留同版本恢复服务、完整网络/合约/DApp 配置及 driver。`MIR2_PROCUREMENT_ENABLED=0` 不能删除既有域配置；仍有待处理任务却缺恢复 driver 时启动拒绝。旧订单使用冻结规则，额度过期不阻断退款/领取。
6. 普通回退不能 DROP trigger、清空 ledger 或换旧 binary。即使资产已结清，删除历史也只能通过另行审计的导出/归档迁移；本轮没有这种任意清账入口。存在未决托管、活证明或待领取时同样禁止删除相关角色。

### 异常恢复操作边界

| 情况 | 核验与恢复动作 |
| --- | --- |
| 网络超时、gRPC 暂时 NOT_FOUND、进程中断 | 保留原业务 ID、签名、资产锁、额度预留和 exact bytes/digest；从持久源恢复，查询原 digest/订单，未知时只重发相同 bytes。不新建订单、不超时退款。 |
| create 确认，activate 未确认 | 空对象仍为 Publishing；不公开、不消费额度。恢复同一激活步骤。只有新鲜、已确认 Clock 超过全部已签发布证明期限且对象仍未初始化，才可补偿。 |
| create 未落链 | 只能使用匹配域/订单、Clock-first、稳定注册表双读和已确认 effects 的 `ChainAbsenceReceipt`；没有已知对象且全部发布证明已失效才释放金币/额度。不得伪造取消终态或对象 ID。 |
| 交易 effects 明确 finalized failure | 记录其 exact digest/checkpoint，原始 envelope 进入失败归档；允许同一步签新尝试。失败交易本身不等于订单失败，不授权退款。未知结果禁止替换 bytes。 |
| 成交/取消/到期竞态、多个卖家 | 仅接受同一订单已确认的互斥终态；原实例转买家待领取，托管金币转卖家待到账，失败竞单原实例返回卖家待领取。重复回调不重复交付/收费。 |
| 交货或取消证明过期/被更高 fence 撤销 | 必须先核验新鲜 Open 对象与已确认 Clock/fence，才退役旧 proof；取消重新确认产生新 intent/task，保留旧记录，取消始终引用原金币锁。不能凭本地时间解锁。 |
| 链上成功，游戏事务失败 | 不回滚链上或凭内存假到账；保留 inbox/outbox，重读最终结果并重试本地原子结算。若本地提交结果未知，冻结 writer、关闭旧 owner 会话，先重载 durable source；禁止补偿扣款。 |
| 买家离线、背包满、金币达到 u32 上限 | 原实例/全部金额保留持久待领取；在线领取使用当前 owner checkpoint，容量足够再原子应用。不截断金额、不生成替代物品。 |
| 事件断线或索引落后 | 从持久 GraphQL cursor 补拉，资产事件和 cursor 同事务去重；无关/空页进度单独持久。索引不可用不阻断 outbox 主动 gRPC 对账。Open 的稳定 watch 负责无人在线时的到期。 |
| Gas 日额度不足或 reservation `.lock` 遗留 | 停止相关 worker，确认原持有进程退出，备份 Gas ledger/作业并核对全部未决 digest。只处理已确认的陈旧锁，保留累计额度和交易历史；不删 unknown 交易或重置预算后盲发。 |

恢复 worker 必须读取同一 durable source。PG 的待处理账号和公开订单从真实 source 发现并刷新；开发 File worker 与 Gateway 共享同一进程/authority，不能另开第二个 File 写者。Windows 实连已验证 Open 订单跨服务重启保留、离线原件待领、三单最终领取及双方结清后再次重登；原生 PG、多机/多 Gateway 并发、长期故障和真实未知提交恢复尚未因此全部通过。
