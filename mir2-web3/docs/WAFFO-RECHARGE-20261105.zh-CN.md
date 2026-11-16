# WAFFO 充值渠道与玩家选择（2026-11-05）

## 范围与当前结果

本轮按用户指定增加 **WAFFO Merchant**，与原有 Stripe 并列，让玩家自行选择渠道；
以后通过同一服务端适配层扩展服务商。Sui Testnet USDC 保留独立路径。
这是商业扩展，不改变原版玩法复刻比例。

代码位于维护分支 `codex/windows-player-journey`，基线
`c404bde8a5190534f6bc83ec8a4fe78fffc8cb50` 加当前未提交工作区。
原有无关 WIP 保留；逐文件来源及本地校验见
[本轮证据](evidence/waffo-recharge-20261105/README.md)。
本轮没有新打包、推送、服务部署、商户开通或实际付款。

## 玩家与接口

- 独立充值网页和共享 Native 法币页从服务端目录显示已启用的 Stripe／WAFFO，
  各自有实际测试／正式模式、报价与 Credits。
- 玩家选择渠道后下单；金额和收款角色仍由认证后的服务端订单约束。
  超时重试继续使用原渠道和订单，不自动切到另一渠道创建第二笔付款。
- 新客户端为所有充值请求发送 `includePaymentProviders: true`。
  只有显式请求扩展信息时才返回 `paymentProviders`、订单 `provider` 和付款结果的
  `provider`，避免已发布 Native 的 `deny_unknown_fields` 拒收新状态。
- 新网页／Native 与新服务端配套使用。新客户端可解析旧状态快照；这不代表带能力标志的
  新请求能在旧服务端执行。USDC 浏览器能力、登录、角色所有权和 Source 租约保持原约束。

服务端目录 `paymentProviders` 每项为 `{id,label,livemode,offers}`。
`billingCheckout`／`POST /v1/billing/checkout` 增加 `provider: "stripe" | "waffo"`；
省略时仍选择 Stripe。未来增加服务商须补适配器、提供者标识及可信收银台规则，
页面布局直接消费目录。

## WAFFO 服务端与账本

- 接官方 Merchant REST：sandbox `https://api-sandbox.waffo.com`，正式
  `https://api.waffo.com`；`X-API-VERSION: 1.0.0`。
  创建、查单使用 RSA SHA256 对原始请求体签名，响应和回调都先验签再解析。
- 使用托管 Checkout；不指定底层 `payMethodType`／`payMethodName`，由 WAFFO
  展示该商户实际开通的支付方式。本版服务器报价先支持 **USD 整数美分**，
  不猜测其他币种精度或本地支付兑换汇率。
- 游戏订单保留 `rch_` 加 64 位十六进制身份；WAFFO `merchantOrderId` 使用其 64 位部分，
  `paymentRequestId` 使用前 32 位，符合官方长度限制。
  `extendInfo` 固定账号、角色、订单、渠道和测试／正式环境，查单逐项核对。
- 首次网络请求前事务性保存 `providerRequestStartedAtMs`。并发重试、进程重启、
  响应丢失后只查原单，不重新发送创建。回调与查单确认 `PAY_SUCCESS` 后才进入原有
  `PaidPending → Applied` 流程；预授权 `AUTHED_WAITING_CAPTURE` 不到账。
- 账本增加缺省为 Stripe 的 `provider`。旧 Stripe 记录序列化时仍省略新字段；
  提供者标识参与订单不可变身份、回调匹配与交易去重。原有 Credits、月卡和存档共同使用。
  整体备份恢复不能删除或回滚已开始的 WAFFO 付款请求。
- 可信跳转限定官方文档明确的 `https://cashier.waffo.com/<非空路径>`。
  猜测的 sandbox 主机、营销示意 `checkout.waffo.com` 和 Pancake 域名均未放行。
  商户真实沙箱返回若使用其他主机，须以官方证据补充三个端的校验并复验。
- 已签名退款／拒付通知核对原单后进入 sticky `Review`，保留已消费存档。
  不把单次退款金额伪造成累计冲正，不自动退款或撤销已使用物品。
  WAFFO 支付 API 没有核实税额时，税额证据保留 `None`，推广分佣保持待核实，
  不把测试资产或未知税额冒充正式可结算收入。

## 商户配置与真实联调的剩余条件

示例：[waffo-billing.env.example](../config/waffo-billing.env.example)，默认关闭。
在现有服务端私人配置中提供：

| 参数 | 来源与用途 |
| --- | --- |
| `MIR2_WAFFO_ENVIRONMENT` | 明确 `sandbox` 或 `production`，不自动升级环境 |
| `MIR2_WAFFO_MERCHANT_ID` | 对应环境的商户编号 |
| `MIR2_WAFFO_API_KEY` | 对应环境的 API key |
| `MIR2_WAFFO_PRIVATE_KEY_FILE` | 商户请求签名私钥文件的绝对路径 |
| `MIR2_WAFFO_PUBLIC_KEY_FILE` | WAFFO 平台验签公钥文件的绝对路径 |
| `MIR2_WAFFO_RECHARGE_OFFERS` | 服务端批准的 USD 报价与 Credits |
| `MIR2_BILLING_PUBLIC_BASE_URL` | 既有 HTTPS 充值服务 origin |

可使用同名去掉 `_FILE` 的 PEM 环境变量，但每把密钥的直接值与文件方式只能选择一种。
密钥不放聊天、源码或公开证据。开启正式环境还要求已有 PostgreSQL SourceOfTruth
账本和明确月卡价格，与 Stripe 的正式存储要求一致。

本轮当前可见消息、仓库和可见附件未提供商户测试参数。填入私人配置并开启沙箱后，
下一步为真实创建、付款、回调验签／查单、到账、重复回调不重复到账，以及平台退款通知。
Gateway、读取账号账本的 Source／worker 与新客户端须配套更新；有 WAFFO 新记录后，
旧二进制不能识别这些新字段，回退需要保留并核对金融记录。

反向代理候选工具新增显式 `--enable-waffo-webhook`，仅把
`/v1/billing/waffo/webhook` 加到已经存在的公开签名回调 matcher。
旧配置逐字保留，普通静态更新不主动开通回调；本轮仅运行了离线候选测试，未执行部署工具。

## 官方规格来源

- [Merchant API 总览](https://waffo.com/docs/en/developer-docs/integration/api/overview)
- [托管 Checkout 接入](https://waffo.com/docs/en/developer-docs/integration/checkout/steps)
- [收银台方式选择](https://waffo.com/docs/en/developer-docs/integration/checkout/customization)
- [Webhook 原始正文验签](https://waffo.com/docs/en/developer-docs/webhook/signature-verification)
- [幂等与未知结果恢复](https://waffo.com/docs/en/developer-docs/core-concepts/idempotency)
- [官方 Go 订单类型](https://github.com/waffo-com/waffo-go/blob/main/types/order/order.go)
