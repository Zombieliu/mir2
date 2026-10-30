# 原生 USDC 直转核账与旧单恢复（2026-10-31）

状态：精确后端源码已部署到原 Windows 测试服，用户指定旧款已恢复为 Applied，实际增加 100 点券。发布前备份、迁移保留和目标发券护栏通过。已核实的公网 r12 安装包仍含旧游戏源码 `1875c6c20`，缺少独立充值域名补丁；客户端点击到手机扫码的完整链路尚未完成发布和人工验收。

## 本轮授权与边界

用户要求解决未完成订单阻塞再次下单，并接通 Dubhe Wallet 普通 USDC 转账自动核账。不启动 Docker；保留既有账号、角色、存档与财务历史；不发布另一会话负责的 Windows 更新器。原服有在线会话时不强制切换，需先正常退出、保存、备份及隔离克隆验证。

用户指定旧订单 `usdc_5ab9455a207912f81ad8923300f2cc33a76a307e2be39be293deeb8ace05d198`，1 测试 USDC / 100 点券。原支付窗口为北京时间 10 月 30 日 23:46:47.307 至 10 月 31 日 00:06:47.307。用户给出的 `CePHU6x45JqU2vuo5yLbQEVUD4TEeuK3AB5X5A1wuvPU` 是 `0xb992…d282 → 下单钱包 0x0725…24ab` 的转账，不是商户充值。另一笔 `8iAt98ntbmNrikLeAh5nV7QES6r3UngBuADPpF85mguu` 才是下单钱包在原窗口内转给商户 `0x98d5…917f1` 的 1 测试 USDC，checkpoint 400944504，付款时间 1793375685597。恢复只能核对后者、归原下单账号，不跨账号转移订单或倒填邀请关系。

## 实现合同

- 原 invoice 订单、收据和旧序列化 / 冻结 hash 保留。新增默认省略的 `paymentMode=direct_transfer` 及独立 typed `transferReceipt`；不伪造 invoice ID、Move 事件或 eventSeq。
- 直转付款人来自当前已验证 Sui 登录 session 的精确 credential，PG 再核未撤销的同账号 credential。客户端不能指定 payer、merchant、金额或发券状态。File/memory 不伪称具备该身份核查能力。
- Node 只读 verifier 绑定 Testnet 完整 genesis、实际 transaction BCS/digest、最终 checkpoint、sender、币种及精确净收款；原 invoice 付款不能成为 direct 付款。无 issuer key、签名、Gas 代付或交易广播。
- 固定官方 GraphQL 仅发现候选 digest；gRPC 独立验真。持久游标按页推进，RPC 未知时不丢候选；索引未覆盖付款截止时间时不宣称窗口已扫描完成。过期后继续发现原窗口内已付款交易。
- 跨 invoice / direct 的 `(chain,digest)` claim 在账号权威事务中唯一分配；源端继续 PaidPending → save CAS → Applied / paid credit lot → 月卡收入来源，不直接改点券余额。
- 过期且从未产生链上创建/付款证据的旧 Prepared 释放容量，保留历史和 request ID 幂等。新直转对同付款人、收款方、币种、金额的未结束重叠窗口拒绝歧义分配。旧未知创建交易仍保留恢复栅栏。
- 旧 Prepared 恢复仅限所有创建/付款字段为空、精确账号 credential 和用户指定订单 / 交易。先验真实链上事实再单向接续 Direct，不延长原窗口；任何失败不伪造付款或强制发券。

## 本地与真实链验证

| 范围 | 新鲜结果 | 证据 |
| --- | --- | --- |
| Gateway USDC / simulation USDC / provenance | 23 / 30 / 15 项通过 | [本地验收](evidence/usdc-native-transfer-20261031/LOCAL-ACCEPTANCE.json) |
| 8 个相邻集成套件 | 40 项通过；显式启用原生 PostgreSQL，未跳过 | 同上 |
| Node / 钱包 UI / Chrome 模拟页面 | 52 / 40 / 7 项通过；operator binary cargo check 通过 | 同上及 [页面模拟验收](evidence/usdc-native-transfer-20261031/BROWSER-MOCK.json) |
| 发布辅助工具守卫 | 分批基础 33 项 + 投影 12 项，共 45 项通过 | [聚合收据](evidence/usdc-native-transfer-20261031/ROLLOUT-HELPER-LOCAL-TESTS.json)、[最终复核](evidence/usdc-native-transfer-20261031/HELPER-PROJECTION-FINAL-REVIEW.json) |

辅助工具 45 项是两组有界测试及最终代码复核，不是同一次 45 项命令；收据保留各组运行时间、源文件 hash 和范围。页面截图属于模拟界面验收，二维码编码的是无付款含义的预览文本，不能当作手机实签或真实付款证据。

真实链只读验收从空游标开始，未提供 digest hint：4 次 `discoverDirectTransfers` 调用、约 30.5 秒找到 `8iAt…mguu`；34 次官方 GraphQL 请求均 HTTP 200，之后 gRPC 独立核实 paid / 1 Testnet USDC / checkpoint 400944504。该步骤没有请求签名、广播、写账号账本或持久化游标。见[自动发现证据](evidence/usdc-native-transfer-20261031/REAL-CHAIN-DISCOVERY.json)。

## 精确 CI 与工件

源码为 `868ef2d2b24de2ab2c577bbe2f3b1667829c9bc3`，分支 `codex/windows-promotion-bridge-20261030`。[GitHub CI 41093332871](https://github.com/Zombieliu/mir2/actions/runs/41093332871) 的 backend 成功，website 跳过。原生 PostgreSQL 验收 54 项通过、0 跳过：相邻付款/归属 40、克隆 CLI 6、钱包身份 8；这些是同一组 54 项的分类，不能重复相加。见 [CI 收据及日志 hash](evidence/usdc-native-transfer-20261031/CI-ACCEPTANCE.json)。

- 工件：`promotion-backend-868ef2d2b24de2ab2c577bbe2f3b1667829c9bc3`，GitHub artifact ID `12997201110`。
- 原 ZIP：89,220,634 bytes，SHA256 `587dbe96b513090ca60dfb07ce85862493231501e9d5e2acf3f123a1b24c63b8`，与认证 GitHub artifact digest 完全一致。
- 正规化 `backend.tar.gz`：85,599,928 bytes，SHA256 `da496f2f6c4d1e3ee0d81b57ef40562e8cd04cf70a018157edeae00963fda5da`，仅含 1,828 个普通文件；只舍弃明确的两个 npm `.bin` launcher 链接。
- 校验器 SHA256 `6559b5d3cb9fefdd693e6322099e179d825898b4c85f0ae17e710012b3fcf487`；公共文件清单 partial 的 hash 为 `84ddbc76f9ab787ade02cc9f03d68ee25856fbe09fce97755dc973367898a050`。认证下载、源码标记、CI 日志 hash/计数及重打包逐文件 hash 均已验证。

见[认证下载](evidence/usdc-native-transfer-20261031/ARTIFACT-DOWNLOAD.json)、[隔离 scratch 中转](evidence/usdc-native-transfer-20261031/ARTIFACT-RELAY.json)、[正规化验证](evidence/usdc-native-transfer-20261031/ARTIFACT-VALIDATION.json)。本机慢下载的片段未当成完整工件；没有在文档中保存临时签名 URL 或私有合并部署合同。

## 隔离克隆与保留护栏

对原备份的隔离克隆验证已通过：125 账号 / 38 角色 / 38 存档；原 26 条迁移保持，新增 `0024_sui_usdc_native_transfer_authority` 后为 27 条，迁移可重复执行。原始备份 SHA256 为 `b501df69eaf6273b0488f8ea8023300ddb5d1c455cd4d4b4585e4f638a4c16b1`。

克隆首次恢复已实际增加 100 点券，早期严格投影比对随后中止。只读诊断证明原始恢复数据、原迁移行和全部非目标数据一致，差异仅限同一保存事务产生的明确投影字段；新迁移元数据只允许 `applied_at` 的恢复时差。诊断没有再运行 operator 或写库。之后沿原克隆前向重放：新增点券 0、唯一 digest claim 1、完整克隆保留摘要不变；没有重新发首次 100 点券，没有删除旧失败证据。见[只读诊断](evidence/usdc-native-transfer-20261031/DIAGNOSTIC-BEFORE-CLONE-PROOF.json)、[前向重放](evidence/usdc-native-transfer-20261031/CLONE-FORWARD-REPLAY.json)。

投影护栏只允许目标 `character_state` 的 credit、save_version 和事务时间变化，credit +100、saveVersion +3 必须与权威存档一致；目标 4 条 `character_items` 仅允许 `updated_at_ms` 同事务更新，所有业务和未知字段保留，时间必须单调且不在未来。其余 37 条 state、219 条 items 的完整摘要保持；原本为空的目标 mail / npc / auction 视图继续为空。没有豁免整表或忽略未知字段。12 项反向守卫见[真实函数本地测试](evidence/usdc-native-transfer-20261031/NORMALIZED-PROJECTION-LOCAL-TESTS.json)。

## 原服发布与旧款恢复

原服在重新核实空闲、备份及克隆验收后完成 `idle_promote`、`verify`、`caddy_exact_asset`、`operator_original` 四阶段。Gateway PID 1177999、promotion worker PID 1178052 均为上述精确源码；切换验证记录 125 账号 / 38 角色 / 38 存档 / 27 migrations，既有 13 条 Stripe 订单与 2 条 USDC 订单保持。Caddy 仅增加精确的 `/v1/billing/sui-usdc/dubhe-wallet.js` 静态路径，其他适配配置保留。见[四阶段收据](evidence/usdc-native-transfer-20261031/ORIGINAL-DEPLOYMENT.json)。

用户指定旧单 `usdc_5ab…d198` 已按真实 `8iAt…mguu` 付款恢复为 **Applied，实际 +100 点券**。付款是原窗口内已有的 1 Testnet USDC；operator 通过共享 Redis 离线 owner fence 和正常 save CAS 发放，保留旧冻结条款、既有字段与目标 4 条物品业务字段。没有新签名、重新广播或继承 issuer secret。见[原服恢复收据](evidence/usdc-native-transfer-20261031/ORIGINAL-RECOVERY.json)。**原服没有重复执行 operator；replay +0 仅是隔离克隆结果。**

运行中只读检查确认 Gateway 的 `MIR2_SUI_USDC_ENABLED=1`、`MIR2_SUI_USDC_DIRECT_ENABLED=1`，Node 和 driver 均指向上述不可变版本，Node 为 v24.22.1。正常 worker 会继续追加活动审计、推进事务出箱，运行后的 `promotion_audit_log` / `promotion_outbox` 不再与发布前全表 hash 相同。数据保留结论依据停服备份与迁移时的严格比对、原订单恢复字段护栏；不声称运行中的整个数据库始终静止，也没有为凑齐 hash 删除日志或暂停正常任务。

18:03:59 UTC 的[最终只读快照](evidence/usdc-native-transfer-20261031/ORIGINAL-FINAL-READONLY.json)再次确认：125 / 38 / 38、六项连接与操作计数为 0，原单仍 Applied / 100、存档净增 100、唯一付款 claim 与关闭的付款窗口各 1，Stripe 历史 hash 不变。该快照只证明更早诊断基线中的 93 条旧审计行完全一致；其后的 16 条为 `worker.internal_projection`，不冒称发布时 105 条审计均再次逐行比对。出箱 11 条数量不变，但字段级状态差异未复核，收据保留此限制。原服没有再执行 operator；克隆 Redis、克隆/诊断库和全部备份保留。

公网 `pay`、`pay.js`、`dubhe-wallet.js` 三次 GET 均 200，规范化源码 hash 与 `868ef2d2…` 一致；no-store、限制 CSP、no-referrer、nosniff 及内容类型通过。该静态检查没有 cookie、授权头、创建订单或操作钱包。见[公网资源验收](evidence/usdc-native-transfer-20261031/PUBLIC-ASSET-RECEIPT.json)。

## 尚待人工验收与范围限制

### Windows 客户端发布交接：充值域名

已核实的[公网 r12 安装包元数据](https://165.154.65.136.sslip.io/client-updates/installers/3b926d5eb5dd8ee1fc46b6d39fba3eda3eb538ba257405733abe1defae840172/release.json)标明游戏为 `WN-CANDIDATE-20261029-unified-11` / `1875c6c20e1285ffefc45ef6c21eab33e26fc536`，游戏 EXE SHA256 为 `85879bfeb1b44d4c759e6a338338bb58a383b7701f886d1090a96b6267f00c6f`。该源码已有下单后打开浏览器的逻辑，但只从 Gateway URL 推导可信充值 origin；测试服返回独立的 `billing-playtest…`，会使原生 USDC 按钮或浏览器 URL 校验拒绝。后端上线不代表这个旧 EXE 已修好。

独立修复为 [`5022a2423ddb0aaf9bb3b5370c68c1862ce31853`](https://github.com/Zombieliu/mir2/commit/5022a2423ddb0aaf9bb3b5370c68c1862ce31853)，只修改 `platform-windows` 的 `billing_browser.rs`、`main.rs`、`session_config.rs` 和两个 candidate 打包脚本。该提交父版本中的这 5 个文件与 `1875c6c20` 一致，可对该旧源码单独 cherry-pick，无需为此合并后端或更换更新器。发布分支若已修改同文件，应逐段合并并保留新工作，不能整文件覆盖。

客户端发布者需将修复后的新 EXE 与以下配置一起打包；旧 `1875` EXE 不识别这个新字段，不能只改旧安装目录 TOML：

```toml
[server]
# 保留该发行包原有 gateway_ws_url、auth_http_url 及其他配置
billing_http_origin = "https://billing-playtest.165.154.65.136.sslip.io"
```

调用现有 `package-windows-candidate.ps1` 时，在原发行参数中增加 `-BillingHttpOrigin 'https://billing-playtest.165.154.65.136.sslip.io'`。它会写入上述字段。充值 origin 必须来自本地发布配置；客户端继续要求精确 HTTPS origin、`/v1/billing/sui-usdc/pay` 路径及有效订单 capability，拒绝其他域名、错误路径及命令注入字符。

r12 launcher 源码 `677e2ab5…` 将 `mir2-client.toml` 列入签名受管文件，正常升级按新文件 hash 在事务中替换，完整安装器同样更新它。应在签名之前把新配置放入包，验收实际安装目录中的新 EXE / TOML；不在签名后手改配置。客户端优先读取 EXE 同目录配置，语言偏好另存于 `%APPDATA%/mir2-web3/locale.json`，无需为本项增加覆盖个人偏好的迁移。

本轮对补丁真实源文件做了[有界本地验证](evidence/usdc-native-transfer-20261031/CLIENT-ORIGIN-LOCAL-VERIFICATION.json)：7 项 Rust 测试通过，覆盖 URL 校验、配置和真实入口配置优先级；打包 profile 真实函数 12 组 / 48 个断言通过。这些测试使用只读已有依赖库，没有重新编译完整主程序、生成新游戏 EXE、启动游戏、操作钱包或发布下载包，不代表新安装/升级后的完整客户端验收。

用户随后明确授权“发过去合入并发布”。补丁、打包参数、签名受管配置说明及验收要求已发送至其既有客户端发布会话「确认 Windows 版是否需要该功能」（`01a18b9b-2c1a-74b0-a305-f9e0ae79640b`），发送工具成功；本记录尚未取得该会话的发布完成回执，不据此宣称新客户端已经可下载。

验收顺序：新 EXE/配置和包签名核对 → 同钱包 F8 登录原服 → 游戏内 USDC 页出现可用档位 → 点支付后自动打开精确充值页并显示该订单二维码 → 用户手机核对并确认一笔 Testnet 付款 → 后台唯一核账、游戏存档到账 → 再次查询不重复发券。当前后端与旧款恢复证据不能代替新 EXE 的这次交互验收。客户端发布仍由用户指定的另一会话负责；本轮不改其安装包、更新频道或下载服务。

新下单后的 Dubhe Wallet 手机实扫、确认新转账、自动发券完整交互仍需用户验收；本轮旧真实付款恢复不能代替这一步。扫码付款的钱包须为下单时已验证的钱包，用户仍须核对 Sui Testnet、USDC、金额与商户地址。原生收款请求中的 requestId 不在链上；归单由精确付款事实、原窗口、无歧义付款窗口及全局 digest claim 共同约束。付款必须发生在原窗口内，过期后的查询只补核截止前的付款。

新权威字段进入 PG 后禁止回滚到旧 strict reader 二进制；所有同库 typed reader 必须同源升级，后续修复沿新格式前向进行。本轮未发布 Windows 客户端或更新器；wallet-login 应用、旧三个活动的独立后端、返利付款开关和官网 Vercel 部署不在本轮变更范围。

证据只保留公共链事实、源码/工件/行集合 hash、计数和布尔断言；账号仅保留 SHA256，未收录私有 operator 输入、连接口令、密钥、会话 token、cookie 或整行账号数据。

## Applied 订单未产生团长佣金：c7fe 个案

用户提供 `usdc_c7fe28d1a3a8f3ee865c30bc7798b00b59649f5a9cd77a0d5273c9d2882ca34c` / `3DH8qxKXRyD2cYX393RTUu4L8dgbQiMk9dwBy4TZsPr5`，询问推广收益页为何没有增加。18:29:47 UTC 的[原服只读诊断](evidence/usdc-native-transfer-20261031/COMMISSION-C7FE-READONLY.json)确认：1 Testnet USDC 已通过原生直转核账 Applied 为 100 点券，目标 paid lot 的 `consumedCredits=0`；没有月卡购买分配、已兑换月卡或月卡收入来源。付款钱包脱敏为 `0xb99297…6cd282`，该账号在当前 Windows 活动里是 approved 团长，没有作为成员确认归属于其他团长。

该活动计佣开启、付款关闭，测试门槛为 1 级 / 1 个 UTC 有效战斗或任务日、准备金 0。角色等级 2，但当前游戏有效日事实为 0；月卡来源、收入分配/挂起、佣金记录均为空，未见与本单相关的出箱投递失败。充值本身只增加点券；须有当前活动有效邀请归属、可追溯付费点券购买并激活的自用月卡及活动资格，才生成团长预估佣金。本轮没有补绑关系、倒填收入、修改资格/金额/规则或打开付款。

源码还明确：页面的 `estimatedCommissionMicroUsd` 是按已激活月卡服务期与赛季的交集折算的赛季预估，不能表述为截至当前时刻的纯现金或已履约实收。`formatPromotionUsd` 保留最多 6 位 micro 小数，并非固定两位把已有微额佣金显示成 0；本单当前确实没有佣金条目。

诊断另记录待修校验缺口：`promotion/store.rs` 普通确认邀请的“已付款”检查只查看旧 USDC invoice `receipt`，尚未覆盖新 `transfer_receipt`。后续真实收入检查仍会把付款早于归属确认的来源置为 `payment_precedes_attribution` review，因此不能用充值后补绑定来使本单自动合法返佣。该缺口不是本单未消费月卡及缺少成员归属的成因；本次只读诊断没有改动它，也不把它作为建议的测试路径。

本地修复补记：已将该门禁统一为 `has_payment_receipt()`，保留 Stripe 判断及原有注册、确认窗口、自邀、已有归属等检查顺序。独立原生 PostgreSQL 先复现直转已付款仍可绑定，再验证修复后的完整归属模块 2 项通过；新增 5 场景覆盖直转 / invoice / Stripe 已付款拒绝、两种 USDC 未付款仍可绑定，并校验拒绝不产生归属或预算预留、不改账号原始账本。见[本地回归证据](evidence/usdc-native-transfer-20261031/PAID-ATTRIBUTION-GATE-LOCAL.json)。此补记仅证明本地修复、尚未部署，记录形成时尚未取得本补丁的精确 CI 与发布收据；未接触远服、链 RPC、签名或真实付款，未启动 Docker。

## 推荐码与有效游玩日：只读归因

18:57:42 UTC 的[只读快照](evidence/usdc-native-transfer-20261031/REFERRAL-PLAYDAY-C7FE-READONLY.json)显示角色 36 等级 2、saveVersion 33，最近一次保存为 18:23:51 UTC。已保存的真实击杀 / 完成任务日志、源事件、投递和有效日事实均为 0，不能归因为后台投递积压。角色仍在线；快照不能证明在线内存中是否刚发生了新行为，需要玩家正常完成真实事件并正常退出保存后再核对。有效日按 UTC 日期去重，一次真实击杀或完成并提交任务即可构成 1 日，无需在线满 24 小时；登录、移动、充值或等级本身均不构成该日。

用户随后提供 `inv_baa…455ae` 的完整旧链接。[19:14:52 UTC 码归因](evidence/usdc-native-transfer-20261031/EXACT-INVITE-BAA9-READONLY.json)确认它在 `numeron-playtest-20261029` 属于 approved 团长钱包 `0x8ced5e…ef2f02`，旧公开邀请接口仍返回 200；当前 Windows 活动 `numeron-windows-playtest-20261030` 中同码数量为 0。不能只修改 URL 的活动参数来转码。当前 Windows 账号仍无候选或最终归属，且已有付款；正常后续测试应由团长提供当前活动的新链接，由新成员在首次付款前确认归属。本轮没有提交候选 / 确认、迁移归属、生成游戏事实、追溯分配充值或中断服务。

本补丁已推送为 `d1d2d82bd99a019e3fd1c739a2cff9114d7959d6`，精确 [CI 41113336591](https://github.com/Zombieliu/mir2/actions/runs/41113336591) 已派发，记录时尚在运行，不能把本地 PG 通过写成新 Linux 工件或原服验收。原服仍为 `868ef2d2…`；后续须核验工件、自然空闲、正常保存及新备份后才能切换 GW/worker。本补丁没有迁移、旧单恢复或 operator 需求，不能重复执行前述旧恢复流程。

用户随后完成真实游戏行为并正常退出。[19:24:29 UTC 退出后只读验收](evidence/usdc-native-transfer-20261031/PLAYDAY-AFTER-NORMAL-EXIT.json)确认：角色等级 2 → 3、saveVersion 33 → 43，保存时间 19:23:14 UTC；真实任务完成和升级事件各 1，源事件均已投递且事实各已入库，当前 Windows 活动有效游玩日为 **1/1**。本次满足有效日的是任务事件，不把升级事件或未观测到的击杀冒充有效日。六项在线计数和 Redis owner lease 均为 0；GW/worker 仍为原 `868ef2d2…` / 原 PID。诊断未补记事实、修改归属或账目，也未停服；这证明真实保存及采集链路正常，不是必须等待满 24 小时。

## 旧活动最终归属的补查与处理路径

用户继续询问如何处理后，新增[19:33:28 UTC 只读跨活动事实核对](evidence/usdc-native-transfer-20261031/LEGACY-CONFIRMED-WINDOWS-LINK-READONLY.json)。同一付款钱包在旧 realm 的账号 SHA 为 `f82b2bb7…`，与 Windows 账号 SHA `9a77de96…` 不同；旧 `numeron-playtest-20261029` **确实已有正式 bound 归属**，指向旧团长 `0x8ced5e…ef2f02`，不是仅有候选。原确认时间 `1793351088394` 早于当前 Windows c7fe 付款 `1793384620286`。此前 Windows 活动没有归属的结论正确，但不能据此推断玩家从未确认过旧邀请；问题包含两个 realm / 活动尚未承接旧关系。

旧团长钱包在 Windows realm 有唯一 active 账号映射，但没有当前 Windows 活动的团长记录或邀请码。当前付款账号则本身已有 approved Windows 团长身份及[现成邀请入口收据](evidence/usdc-native-transfer-20261031/WINDOWS-CURRENT-LEADER-LINK-READONLY.json)；邀请码不是本轮新建。角色等级 3、100 点券和有效日 1/1 均保留。

当前源码没有跨 realm / 活动归属导入、管理员重绑或迁移审批接口：归属主键和团长外键按活动限定；普通 `confirm` 的已付款门禁仍成立；改 URL 或复制记录不构成合法迁移，`Reconcile` / 申诉也不会创建新活动的归属。给用户的两条具体路径为：①保留原团长→团员关系，新增明确允许来源、钱包一致性、原始凭据、当前条款、冲突/去重/名额与不可变审计的迁移流程，原始确认时间与实际迁移时间分开记录，不能倒填普通确认或自动追认收入；②不迁移旧关系，当前已充值账号继续作为 Windows 团长，让另一未在 Windows 付款的钱包通过现成邀请入口先本人确认，再充值并购买激活月卡。两者都不删除或重建现有角色及账本。记录时已请求用户确定处理路径，尚未选择或执行任何迁移、补绑、换角色、计佣追认或付款操作。

## 已授权的旧邀请迁移：实现与待完成条件

用户随后明确选择沿用原团长和团员关系，并要求原始凭据与审计。本次新实现仅允许 `test` 环境、旧 `numeron-playtest-20261029` → `numeron-windows-playtest-20261030`，由可信 CLI 从两个独立 PostgreSQL namespace 读取原始记录；浏览器不能上传归属证据。输入须指定两个完整钱包，源与目标各有唯一 active 身份，原始最终归属、候选、确认审计的团长、条款、注册与点击时间必须一致；源端只读 repeatable-read，拒绝旧确认前付款、已有源收入分配和跨 realm 重复的付款凭据。

准备需要现有 Operations 与独立 Finance 角色、复核的 snapshot hash、幂等键和证据引用。只追加显式 `migration_import` 候选、不可变凭据与审计，不建立最终归属、不预留名额、不认领付款。原团长须先本人完成当前 Windows 活动条款与报名；团员之后通过当前已验证钱包的正常 `confirm` 接口接受当前条款。旧条款不能充当新活动的接受记录。

最终确认保留原注册／确认窗口、活动状态、真实当前条款、自邀、冲突、名额与预算检查。`confirmed_at_ms` 写实际本次确认时间，原确认时间单独保留；资格锚点取旧确认、目标双方注册、目标活动开始的最大值。只有准备时逐笔核验且未在源 realm 出现的明确付款收据，才能在本次真实确认后使用该锚点；未列明的历史付款继续执行原先的早于归属挂起规则。付款唯一 claim 仅在本人最终确认事务中写入。SQL 新增第 28 条迁移和三张追加式表，账号 JSON 仍为 v14。

网页增加“旧站邀请迁移 · 待本人确认”与最终确认凭据视图，独立勾选当前条款；请求成功只表示已提交，刷新取得真实归属后才显示完成。提示支持现有 10 种语言，并隔离账号、凭据或迁移证据切换后的迟到响应。真实充值、月卡消费和激活、真实任务／战斗日、收入分配及付款继续使用原权威链路，迁移不创造这些事实。

发布要求 Gateway 与 worker 同源升级。旧 `868ef2d2…` worker 不理解迁移资格锚点，不能在迁移使用后单独退回；本次不复用旧订单恢复 operator、不更新 Node／Caddy、不发布 Windows 客户端、不打开返利付款。普通已付款确认门禁提交 `d1d2d82b…` 的 [CI 41113336591](https://github.com/Zombieliu/mir2/actions/runs/41113336591) 已成功，但不包含上述迁移，不能当作本次新源码的发布工件。

本地新迁移[原生 PostgreSQL 14 项](evidence/usdc-native-transfer-20261031/PROMOTION-MIGRATION-PG-LOCAL.json)通过，0 跳过／忽略／过滤；包含来源审计语义、原始注册／候选一致性、准备无归属或预留、本人确认、既有付款例外、过期重新准备、追加式约束与实际 worker 出箱推进。7 个相关源码执行前后 canonical-LF hash 一致。另有[CLI 5 项及 6 个相邻套件 16 项](evidence/usdc-native-transfer-20261031/PROMOTION-MIGRATION-ADJACENT-LOCAL.json)通过；最初发现并修复了两个测试对“最后一条迁移为 0024／总数 27”的陈旧假设，旧失败日志仍保留，修复后重新运行完整相邻组。这些均为本机独立 PG 和明确模拟付款凭据，不代表真实链付款或原服迁移完成。

用户之后在正确 Windows 活动完成报名。[20:12:30 UTC 原服只读复核](evidence/usdc-native-transfer-20261031/PROMOTION-MIGRATION-TARGET-LEADER.json)确认原团长钱包对应的精确目标账号已有 approved 记录，当前条款 `numeron-windows-playtest-20261030-v1` 接受时间为 `1793391026068`，即北京时间 10 月 31 日 04:10:26。该记录由用户真实操作建立，未在后台补建。本段记录实现、验证和前置条件；新源码精确 CI、隔离克隆、同源发布以及团员本人最终确认仍待完成，尚未迁移真实关系。
