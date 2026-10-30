# 原生 USDC 直转核账与旧单恢复（2026-10-31）

状态：实现与验证进行中，尚未部署或改动原服订单；不得将本记录当成上线验收。

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

## 发布与验收

待最终测试和精确源码 Linux 工件。原服预检为 Gateway / promotion worker `050950f5…`，124 账号 / 38 角色 / 38 存档 / 26 migrations，仅是当时快照，切换前重新取基线。新字段进入 PG 后不得用旧 strict reader 回滚；Gateway 和同库 worker 必须同源更新。wallet-login 应用、旧活动后端、返利付款开关和客户端渠道保持本轮范围之外。

本地新鲜验证：Gateway USDC 23/23；simulation USDC 30/30、provenance 15/15；8 个集成套件 40/40（启用真实原生 PostgreSQL，未跳过）；Node 52/52；钱包 UI 40/40；Chrome 页面 7/7；operator binary cargo check 通过。真实链只读核验已确认 8iAt… 为 paid，但尚未修改原服账本。`evidence/usdc-native-transfer-20261031/` 保存页面模拟验收，二维码仅编码无付款含义的预览文本。Linux CI、隔离克隆和原服入账仍待执行。
