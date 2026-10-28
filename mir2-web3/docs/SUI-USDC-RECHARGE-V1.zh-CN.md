# Sui 原生 USDC 充值第一版

日期：2026-10-29（Asia/Shanghai）。本记录对应独立开发分支中的实现与下表所列验证；不代表远程测试服已更新、主网开放或真人最终验收。

## 已确定的产品边界

- 点券充值同时保留 Stripe，并增加 **Sui 网络的 Circle 原生 USDC** 通道。本版不支持用 SUI 币充值；SUI 仅承担链上 Gas。
- 充值结果进入现有游戏服务器点券钱包。个人智能求购仍使用链下金币购买装备，独立求购服务额度的收费规则不随本功能改变。
- 套餐价格和点券数量来自服务器现有 `RechargeOffer` 配置，浏览器不能指定金额、点券数、收款人或最终支付状态。
- 当前只允许隔离环境与 Sui Testnet。主网、生产收费、真实资金、生产密钥和远程发布均未获本轮开放。
- 测试网 USDC 到账明确记录为 `Test`，不能计为现金收入或自动转换为已付费求购额度。

## 框架与复用范围

充值合约是独立的 Sui Move 发票模块；现有 Dubhe 个人求购合约保持独立。这次充值模块没有使用 Dubhe schema/system/Marketplace，不把普通 Sui SDK 调用描述为 Dubhe 接入。

采用 `@mysten/sui` **2.16.2** 与 gRPC；Sui framework 固定为 `3c0f387ebb40b8be292d3b7bd3f5bee8ad226d33`，编译器为 **1.68.1**。具体上游地址、校验依据和排除项见 [UPSTREAM.json](../onchain/usdc-recharge/UPSTREAM.json)，合约/CLI 协议见[充值模块说明](../onchain/usdc-recharge/README.md)。

| 现有代码 | 本次复用 |
| --- | --- |
| `simulation/src/billing.rs` | `BillingLedger` 增加独立 USDC 订单；继续使用同一角色点券钱包、到账回执与容量检查 |
| `runtime/billing.rs` | 在线 Source 串行入账、save revision CAS、5 秒轮询、显式 `BillingRefresh`、`GainedCredit` |
| Gateway `web_billing.rs` | 真实账号/角色验证、限流、在线/离线路由权限、已有 Stripe 通道 |
| Windows 原生充值面板及 browser opener | 通道选择、订单状态、同源浏览器交接、重登后恢复同一订单 |
| account-store 事务与迁移 runner | File 单写权威、PostgreSQL 原子写入、未知提交结果分类、升级与降级保护 |

Circle Testnet 原生币种严格固定为：

```text
0xa1ec7fc00a6f40db9693ad1415d0c193ad3906494428cf252621037bd7117e29::usdc::USDC
```

精度为 6；链标识为 `4c78adac`。主网参考地址不能作为配置替换项，也不接受桥接币或名称相同的其他 coin type。

## 一笔充值的完整流程

1. 玩家在 Windows 充值面板选择 USDC 套餐。认证后的 Gateway 冻结账号引用、角色、套餐、网络、package/config、商户、币种、金额、规则版本和期限，写入持久订单。
2. 原生客户端仅打开与本地 Gateway 配置完全同源的充值页面。URL fragment 携带这一张订单的短期能力凭证；不携带游戏登录 bearer。页面立即清除地址栏 fragment，不把能力凭证存入 localStorage。
3. 钱包先签个人消息，确认订单、商户、金额、点券数、网络、期限和网页来源。服务器验证签名，永久绑定 payer。连接钱包或登录游戏本身不等于同意付款。
4. 后台创建发票：先保存 **精确签名交易字节、签名和 digest**，然后才广播。每张订单绑定一个最终确认的 shared Invoice 对象；所有订单复用同一合约代码，不逐单部署合约。
5. 玩家钱包再次明确确认付款。合约核对 payer、商户、金额、原生 USDC 类型、订单条件、发票未付状态和链上 Clock。付款转账与标记已付在同一 Sui 交易中完成。
6. 后台直接查询全节点，验证最终成功 effects、精确事件、发票条件和一致的对象版本；客户端提供的 digest 只用于提示查询，不能触发发点券。
7. 数据库先原子记录 `PaidPending` 及永久支付回执。在线 Source 或离线账号权威随后在同一数据库事务中增加点券并写 `Applied` 回执。界面区分“链上确认”和“游戏内到账”。

同一 USD 套餐的金额按 `amount_minor × 10,000` 得到 USDC 微单位，使用整数与溢出检查。例如**已配置**的 1,000 美分套餐对应 10,000,000 微单位；点券数量沿用该套餐的 `credits`。该示例不是新增套餐定价，不假设固定美元兑换点券比例。

当前订单付款期限为 20 分钟，浏览器能力凭证期限为 30 分钟；它们不是永久授权。凭证过期或关闭新充值不影响已经确认付款的到账。链上付款时间在期限以内、后台确认稍晚时，仍按原订单交付。

## 一致性、权限与恢复约束

`Prepared → PayerBound → InvoicePrepared → InvoiceBound → PaidPending → Applied` 是持久状态机；明确创建失败或核验异常可以进入 `Review`。`Review` 不是超时退款或重新发行发票的授权。

- payer 一经绑定不可改，且不能与 merchant 相同；所有订单引用与冻结条件均参与校验。
- 网络、合约、发票、订单、账号引用、payer、merchant、coin type、金额、冻结哈希、支付 digest、事件序号和最终 checkpoint 必须匹配。
- 多次请求、后台重试、重复回调、服务重启不产生第二张发票或第二次点券入账。确定未提交的数据库失败可以用同一订单重试；提交结果未知时先对账。
- 点券为 `u32`。容量不足时整笔保留为 `PaidPending`，不截断、不丢弃、不按较小数量到账；钱包腾出空间后重试。
- 服务端轮询具体发票及创建交易，不依赖实时事件或索引器。索引器和余额变化均不是单独的交付依据。
- 后台按完整 `(accountId, orderId)` 游标分页，游标先于页大小过滤；空页后回绕。已经确认付款的订单交给 Source，不反复占用链上查询页。
- 已经签发创建字节的订单即使超过本地期限，仍保留对账。只有从未签发创建交易的过期订单才能停止链上发现。
- 同一 `(chain_identifier, config_id)` 同时只有一个未最终确认的创建交易，防止发行方并发选中同一 SUI Gas 对象。120 秒处理 lease 不等于该创建栅栏的有效期。
- 创建栅栏仅在确认发票或最终 `createFailed` 后释放；超时、未知、lease 到期及无确认发票的 `invoiceMismatch` 都不能释放。后台优先恢复既有创建，同时轮转其他发票。
- 合约 `AdminCap` 可创建发票和暂停新发行，不能修改既有发票或直接增加游戏点券；`UpgradeCap`、商户资产保管和服务器数据库仍是信任边界。

游戏服务器是可信商户和点券交付方。合约不能强迫离线服务器交付点券，UI 和文档不宣称完全无信任交易。

## 运行配置与开放开关

所有敏感值应通过隔离环境注入，不提交私钥、checkout key、完整账号库或带能力凭证的 URL。

| 配置 | 含义 |
| --- | --- |
| `MIR2_SUI_USDC_ENABLED` | 默认关闭；`true/1` 才允许新订单。关闭新充值时仍保留恢复驱动配置 |
| `MIR2_SUI_USDC_DRIVER_CONFIG` | 指向受控 JSON；必须 `environmentTag=isolated-test`、`network=testnet`，冻结 package/config、发行方、商户和原生币种 |
| `MIR2_SUI_USDC_OFFERS_JSON` | `RechargeOffer[]`，至多 12 套；只允许 USD 价格源。`credits` 不由客户端计算 |
| `MIR2_SUI_USDC_RULE_VERSION` | 正整数，默认 1；订单固定所采用版本 |
| `MIR2_SUI_USDC_PUBLIC_ORIGIN` | 与 Windows 所配 Gateway 导出的来源完全一致；正式格式为 HTTPS，本机隔离允许 `http://127.0.0.1:端口` |
| `MIR2_SUI_USDC_CHECKOUT_KEY` | 32 字节、64 个十六进制字符；生成单订单 HMAC 能力凭证。已有订单恢复期间保持稳定 |
| `MIR2_SUI_USDC_NODE` / `MIR2_SUI_USDC_DRIVER` | 绝对路径；Gateway 校验驱动及锁文件的编译时源码指纹 |
| `MIR2_SUI_USDC_ISSUER_SECRET_KEY` | 明确的隔离测试 Ed25519 `suiprivkey`；不自动生成、不读默认钱包、不送浏览器 |
| `MIR2_SUI_USDC_GAS_LEDGER_PATH` | 发行方创建交易的持久 Gas 预约账本绝对路径；跨子进程/重启继续使用同一账本 |
| `MIR2_SUI_USDC_GAS_MAX_MIST` | 创建交易的累计预算上限，默认 50,000,000 MIST；账本固定网络/config/发行方及上下限，不能通过重启重置预约 |
| `MIR2_MONTHLY_CARD_CREDIT_PRICE` | 保留现有 Source 首笔外部充值轮询门禁；Gateway 与所有 Source 必须一致配置，不改变原有月卡价格 |

驱动 JSON 的完整字段见[CLI 说明](../onchain/usdc-recharge/README.md#cli-protocol)。`gasBudgetMist` 必须为正数且不超过 50,000,000；这是单次创建交易上限。广播前另按 digest 在持久账本预约整笔上限，同一 digest 原样重放不重复预约，未知结果不释放，锁冲突或损坏账本默认拒绝。Node 子进程调用白名单、输入/输出大小限制和 20 秒外层截止时间均由 Gateway 约束。

`MIR2_RUNTIME_ENV`、`MIR2_DEPLOYMENT_ENV`、`MIR2_ENV` 全部参与环境门禁；去除首尾空白、转小写后，任何一个标记为 `production/prod/staging/stage/preprod/preproduction` 都拒绝，不可用另一个 development 标记覆盖。不可读取的环境值也拒绝；关闭新充值的恢复模式同样受此约束。要部署到公共或生产环境，需要单独的开放、密钥保管、Gas 总预算、数据库和发布验收。

## 数据库迁移与回退

Account-store schema 从 12 升至 **13**。普通不带 USDC 权威字段的旧数据按规则升级；版本标签低于 13 却携带新 USDC 订单的输入被拒绝，不能借迁移“认证”伪造历史。

新增[迁移 0019](../infra/postgres/migrations/0019_sui_usdc_billing_authority.sql)，由现有迁移 runner 在 advisory migration lock 内执行：

| 数据表 / guard | 作用 |
| --- | --- |
| `sui_usdc_invoice_receipts` | `(network, invoice)` 唯一，永久绑定账号和订单 |
| `sui_usdc_payment_receipts` | `(network, digest, event_seq)` 唯一；同一 invoice、账号订单各只能绑定一个付款回执 |
| `sui_usdc_active_creation` | `(network, config)` 唯一的未确认创建栅栏，无超时自动释放 |
| `accounts_sui_usdc_billing_authority_guard` | 在账号权威事务内注册/释放栅栏、保存回执；禁止删历史、改冻结条件或替换已绑定 payer/创建字节/发票/付款/到账回执 |

这些表是同步权威栅栏，不是异步读模型；后续钱包提交失败时，账号快照、回执和创建栅栏一起回滚。迁移重跑幂等，并对已存在的 13 版记录回填注册。

回退步骤：

1. 关闭新充值，保留相同版本的 Gateway/Source、驱动、冻结域和既有能力凭证密钥。
2. 完成所有已签发创建交易及已付未到账订单的对账，保持玩家可刷新和领取原有点券。
3. 保留 13 版权威字段、`0019` guard 与唯一性表。不能通过删表、清回执、改余额或替换发票“回退”。
4. 降至旧程序必须另做审计导出/归档和专门迁移；不是普通二进制回滚。本次没有执行数据库降级或远程迁移。

File 模式沿用单进程写入 authority 和原子文件替换，仅用于本机隔离/工程验证；多机写入验收必须使用原生 PostgreSQL，不能把 PGlite 结果当成该验收。

## 异常恢复操作

| 现象 | 应对 |
| --- | --- |
| 创建交易广播超时或服务崩溃 | 找回订单已持久化的创建 digest，使用原始字节查询/重试；不重新签另一笔创建，不清 `active_creation` |
| lease 到期、另一进程接管 | 接管原订单和原始交易；数据库创建栅栏继续存在，直到确认终态 |
| 付款已成功，数据库事务失败 | 重新核对同一发票的最终回执，幂等写 `PaidPending`；玩家不需要再付一次 |
| `PaidPending` 持续存在 | 检查当前 Source 所有权、save revision、钱包上限和普通 BillingRefresh；不绕过 owner 直接改钱包 |
| Node/RPC 暂时不可用 | 显示结果待确认并保留持久状态；恢复驱动后继续对账 |
| 关闭新充值或凭证已过期 | 对账与已付款到账继续；凭证不能无限授权新交易 |
| 明确失败的玩家付款交易 | 不把 digest 当付款成功。当前同单重新签名恢复策略须在真实钱包流程中单独验收；不能只因超时清提示后让玩家再次支付 |

运维排查只记录订单 ID、digest、checkpoint、稳定错误码和状态。签名交易原文、能力凭证、密钥和游戏登录令牌不得放入公开验收包。

## Gas 与非 Gas 成本

成功充值设计为 **1 笔发行方创建发票 + 1 笔玩家钱包付款**。合约部署、config 初始化和启用新发行是独立设置交易，不逐单重复部署。

当前发行方为创建交易支付 Gas；玩家钱包为自己的 USDC 付款支付普通 SUI Gas，界面必须在确认前展示。自定义 Move 调用不沿用受限的 gasless stablecoin transfer 路径。本版没有开放任意交易代付。

一次真实 Circle Testnet USDC 充值已完成链上与数据库核验：[独立审计](generated/sui-usdc-recharge-v1-20261029/live-real/final-recharge-audit.json)。成功订单收取 **1 测试 USDC**、发放 **100 测试点券**；这是显式隔离测试套餐，不是正式充值比例审批。

| 成本 | 实测 MIST | 备注 |
| --- | ---: | --- |
| 成功单创建发票 | 4,815,356 | 发行方承担；交易 `6Kmz6ohmUNmpEipaZetWuR5HvZnTy93EkfhnyeQUKZvV` |
| 成功单 USDC 付款 | 2,322,914 | 钱包承担；交易 `E2yqrhVNuVPCWcvJkmW93fbXfHAfRLkqXyTqiTcdA4SJ` |
| **成功单两笔合计** | **7,138,270 = 0.007138270 测试 SUI** | 不含设置、旧失败尝试或服务器成本 |
| 旧过期、未付款发票 | 4,815,356 | 外部 QA 钱包 helper 在签名前拒绝，不能记为成功充值 |
| 独立设置成本 | 30,550,599 | publish 25,677,200 + create_config 3,872,956 + enable 1,000,443；不逐单重复 |

Gas 按 `computationCost + storageCost - storageRebate` 计算，不再次加已包含的 non-refundable fee。服务器、RPC 与运维费用未测量；测试币不折算现金收入，也不能据此推出正式套餐利润。此前个人求购约 `0.0341` 测试 SUI/单是另一功能的数据。

## 验收与证据状态

| 验证项 | 2026-10-29 当前结论 |
| --- | --- |
| Rust USDC 状态机 | **21/21 通过**；含两个账号并发创建栅栏、未知提交重启、8 线程重复确认、溢出待到账、来源与全部冻结条件、游标跨账号恢复 |
| 同一新二进制 Billing 过滤集 | **24/24 通过**，包含上述 21 项及 3 项相邻检查，不与 21 再次累加 |
| 个人求购相邻回归 / schema | **54/54 / 3/3 通过** |
| SQL `0019` | **PGlite PostgreSQL 18.6 WASM 31/31 通过**；不是原生 PostgreSQL或真实多连接并发 |
| Gateway HTTP/后台恢复 | v3 **13/13**；v4 过期展示修复后 **15/15**；相邻 Billing **21/21**、Stripe **4/4**。v4 新增精确到期边界和未知/已付状态保护 |
| Windows 原生逻辑 / GPU | **14 项逻辑检查**；独立 GPU 测试 **1/1、10 张离线原生状态图**；host/URL/作用域 **7/7**。逻辑过滤命令里的原 1 个 ignored GPU 用例已通过独立 GPU 命令实际执行，不记成无覆盖 |
| Move / Node | **14/14 / 22/22**；Node 最终日志是 `node-test-lifetime-gas.log`，原 `node-test.log` 保留 16 项历史。完整版本和边界见[链上验证记录](../onchain/usdc-recharge/validation/README.md) |
| 真实测试网创建、Circle USDC 付款、数据库到账 | **已通过独立全节点+数据库核验**：买家 19.99→18.99 测试 USDC，商户 0→1，唯一 `Applied` 回执与点券 0→100。操作使用显式 `Isolated QA Wallet (real Testnet)`，不是普通钱包扩展的真人最终接受 |
| Windows 真实操作 / 重登 / 真人最终验收 | **真实 Windows 已验证**：0→100 点券、重复刷新保持 100、普通 GameShop 显示点券 100/金币 10,000；v4 服务重启及正常重登后仍为 100，关闭新充值后两张旧单显示过期、成功单显示已到账。使用隔离 QA Wallet，**普通钱包扩展/真人 Accepted 仍开放** |
| 远程测试服 / 主网 / 生产收费 | **未由本轮发布 / 未开放 / 未开放** |

已脱敏日志、Mock 状态图、SQL 可重跑验证器、二进制元数据与源码指纹归档至[本专题验收包](generated/sui-usdc-recharge-v1-20261029/README.md)。真实测试网核验、六张实际 Windows 交互截图、Mock 和旧失败尝试分目录保留。成功付款发生于 v3 Gateway；v4 验证重启、正常重登及关闭新充值后的恢复，不能改记成 v4 产生付款。重复提交同一真实 digest 两次及状态查询在 v3/v4 均保持唯一到账、余额与回执不变。

## 主要修改文件

| 模块 | 文件 |
| --- | --- |
| 模拟服权威 | 新 `apps/simulation/src/sui_usdc_billing.rs`、`sui_usdc_billing_tests.rs`；整合 `billing.rs`、`runtime/billing.rs`、`config.rs`、`db_projection.rs`、`lib.rs`；采购 schema 常量和两处版本断言随 account schema 更新 |
| Gateway | 新 `sui_usdc_billing.rs`、`sui_usdc_billing_tests.rs`、`web_sui_usdc.rs`、`sui_usdc_checkout.html/js`；整合 `web_billing.rs`、`web.rs`、`lib.rs`、`main.rs` |
| Windows | `client-bevy/src/native_billing.rs`、`native_i18n.rs`；`platform-windows/src/billing_browser.rs`、`gateway.rs`、`main.rs`、`native_protocol.rs`、`shell_bridge.rs` |
| 链上 | 新 `onchain/usdc-recharge/`：Move 合约、固定 SDK/lockfile、严格 CLI/验证器、测试与上游/验证记录 |
| 数据库 | 新 `infra/postgres/migrations/0019_sui_usdc_billing_authority.sql` |
| 文档 | 本记录；整体当前进度与实际 Windows 验收由相应协调记录更新 |

## 仍需确认或完成

- 审批具体充值套餐、商户与正式环境；本次没有改任何既有经济数值。
- 真实测试网付款、数据库/Windows 到账、重登和 Gas 已有记录；普通钱包扩展及真人最终接受仍需完成。测试资产来源明确，不绕过 faucet 的人工验证码。
- 原生 PostgreSQL 事务、两进程/多主机竞争、重启与实际 RPC 故障恢复门禁。
- 正式发行密钥、UpgradeCap 管理、创建 Gas 总预算、限额/告警和收费开放审批。
- 现有总点券余额尚无完整可消费的“付费/赠送/测试”批次拆分；USDC 账单的明确 Test 来源不会补齐历史混合点券溯源，也不会自动解锁生产求购服务收费。
