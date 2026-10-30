# 原生 USDC 直转核账与旧单恢复（2026-10-31）

状态：精确源码已部署到原 Windows 测试服，用户指定旧款已恢复为 Applied，实际增加 100 点券。发布前备份、迁移保留和目标发券护栏通过；新订单的手机扫码付款尚未取得人工验收。

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

新下单后的 Dubhe Wallet 手机实扫、确认新转账、自动发券完整交互仍需用户验收；本轮旧真实付款恢复不能代替这一步。扫码付款的钱包须为下单时已验证的钱包，用户仍须核对 Sui Testnet、USDC、金额与商户地址。原生收款请求中的 requestId 不在链上；归单由精确付款事实、原窗口、无歧义付款窗口及全局 digest claim 共同约束。付款必须发生在原窗口内，过期后的查询只补核截止前的付款。

新权威字段进入 PG 后禁止回滚到旧 strict reader 二进制；所有同库 typed reader 必须同源升级，后续修复沿新格式前向进行。本轮未发布 Windows 客户端或更新器；wallet-login 应用、旧三个活动的独立后端、返利付款开关和官网 Vercel 部署不在本轮变更范围。

证据只保留公共链事实、源码/工件/行集合 hash、计数和布尔断言；账号仅保留 SHA256，未收录私有 operator 输入、连接口令、密钥、会话 token、cookie 或整行账号数据。
