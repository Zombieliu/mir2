# Numeron 团长推广实现与验收记录

依据用户提供的《Numeron 团长推广网站实施方案 v1.0》及后续确认实施，更新于
2026-10-29（北京时间）。本轮已接入真实游戏持久化事件、付费点券 FIFO 与实际月卡
权益来源，并增加受审核的 Sui 付款执行和恢复程序。生产开关已从永久禁止改为明确
配置、证据和双人审核门禁；不能把本地测试通过写成真实手机、Stripe 或链上验收完成。

2026-10-29 已在原官网完成一次真实 Sui Testnet 充值返利闭环，详见下节；没有提交
Mainnet 交易。手机端付款交易签名、Stripe 与独立人工财务审核仍不在本次自动验收
结论内。没有启动 Docker，也没有继续 Windows 自动更新发布。

## 真实充值到测试网返利闭环（2026-10-29）

入口：[原官网测试赛季结算](https://mir2.obelisk.build/promotions?campaignId=numeron-chain-acceptance-20261029-fullchain#earnings)。
这是独立15分钟验收赛季，北京时间15:24:09.320开始、15:39:09.320结束；没有改时钟、
回填注册时间、注入玩法或付款事实，也没有缩短月卡真实30天服务期。

| 环节 | 本次真实结果 |
| --- | --- |
| 网站与后端 | 原 Vercel 项目发布 `78fac20b3`（`dpl_Hmab1y6vYULtdHJZ4Zy6fsWwMbzm`）；独立原生后端 `4151a63d5`，24项增量迁移，SDK2.16.2与Node24运行时有固定校验和 |
| 邀请 | 官网真实登录／签名交换；1次邀请点击后新注册、确认归属，最终1名 `qualified` 成员 |
| 普通游戏 | `WarmGame1`、`MiOct29` 正常建角、战斗、升2级、退出存档；权威存档、事件、投递ACK、推广事实一致，待消费事件为0。独立目录部署1620张原始地图并逐项验哈希 |
| 真实充值 | 已授权测试付款钱包转出10.000000官方测试USDC，商户余额恰增10，1000点券最终 `applied`；摘要 `FQzCWJjhofsBwgBsUSsE3i4mZ76UXqFrvMFy48pqMcjw`，检查点 `400452807`，仅广播一次 |
| 月卡与来源 | 普通HTTP刷新领取到账点券，花费1000付费点券购买并激活30天月卡；FIFO来源保留 `sui_native_usdc`、Testnet、合约、币种、发票、金额、摘要及检查点，赠送／未知来源点券均为0 |
| 核账 | 月卡激活于15:38:54.085，实际覆盖本季15235毫秒；30天收入按交集折算为58测试微USD，首档10%佣金为5测试微USD，准备金0。封账后原生离线复算与不可变快照一致 |
| 真实返利 | 向本次测试团长钱包 `0x01929c47553bc2541c1164238199b6226ccaef2bd9308bd81ba0bc0420511042` 返0.000005测试USDC；摘要 `eRhD2reuEHyE1cNFLY22598uKWJPYdvCcS7T9cgCU7x`，检查点 `400454295`。数据库和链上均 `confirmed`，收款余额恰增5个原子单位 |
| 执行顺序 | 测试operations冻结意图、不同finance身份审核；签名先持久化，PG保留原摘要并标记提交后才广播一次。两个自动角色不代表独立真人财务审核 |
| 数据保留 | 原游戏119账号、35角色、35存档及3个进程PID保持不变；独立测试库现为10账号、2角色、2存档。原七天活动仍招募／计佣开启、付款关闭；11月至2月草案仍未开启 |

金额极小来自真实服务覆盖时间折算，既不是50USDC算例，也没有技术赠款或最小金额补足。
网页实测已显示精确金额、已确认状态、摘要与检查点；公共入口可以沿用已验证的钱包
进入角色选择，不要求浏览器钱包插件。本次实际交易使用已授权隔离测试密钥，
不能据此宣称手机钱包交易签名、主网付款或Stripe已验收。

持久证据：[闭环报告](evidence/promotion-fullchain-20261029/FULLCHAIN-ACCEPTANCE.json)、
[独立链上复核](evidence/promotion-fullchain-20261029/CHAIN-INDEPENDENT-VERIFICATION.json)、
[官网结算截图](evidence/promotion-fullchain-20261029/FINAL-SETTLEMENT.png)。原始无密钥收据
保存在本机 topic `mir2-promotion-live-20261029/fullchain/`，报告固定13份收据的SHA256。
后端回归包括推广44项（7个原生PG场景）、游戏授权及相邻32项；网页恢复27项与完整
TypeScript检查通过，签名前完整交易校验16项通过。这里只认证本次Testnet闭环，
项目整体Accepted仍需人工前端验收。

## 复用与边界

- 网页入口为 `/promotions`，邀请页为 `/promotions/invite/<code>`，位于现有
  `apps/web`；复用其 Next.js、React、qrcode 与 Sui 签名验证依赖，没有建立第二套玩家库。
- 登录沿用现有 Dubhe Connect v2 协议、relay 与 `dubhe-connect-auth.ts`。
  nonce、站点、网络、签名及过期验证由既有真实适配器完成；新 BFF 把挑战绑定到
  浏览器 HttpOnly Cookie，网关沿用共享缓存单次消费 proof。
- 复用 Rust `IdentityService` 的可撤销身份会话及真实游戏账号、角色、恢复守卫。
  渠道兑换用的短期 gateway token 与网页身份 session 是不同类型，不能互换。
- 原密码账号可以先证明密码控制权，再以新的钱包签名绑定。不会按昵称、请求中的
  account ID 或截图认领角色。钱包已绑定另一账号时拒绝，进入恢复支持流程。
- 网页钱包校验只调用权威账号预检、恢复及新账号创建边界，不执行原生
  `PasskeyLogin` 的密码修复操作。绑定期间再次校验原会话，数据库行锁内拒绝过期、
  已撤销或账号不匹配的凭据。新钱包锁定标记使用 PostgreSQL 可保存的无空字符值，
  仍拒绝历史文件锁定标记的密码登录。
- 共享的受信钱包登录对已绑定的普通 Argon2id 密码账号也保留其密码，并在写事务中
  再检查一次，避免稍后使用游戏钱包登录时覆盖原凭据。钱包保留命名空间与历史弱默认
  记录仍执行原有锁定修复；没有改动 Windows 客户端或推进自动更新发布。
- `promotion` 模块通过不可变游戏账号 ID 关联原账号与角色。来源事件和付费批次随原
  权威账号／存档 CAS 提交；推广、财务、审核及付款记录另存 PostgreSQL 业务表。
  本轮没有清空或重建原账号、角色及余额。

沿用现有 `package-lock.json`：Next16.4.1、React19.2.3、`@mysten/sui`2.16.2、
qrcode1.5.4、TypeScript5.9.3。Dubhe 集成采用仓库已有的 Connect v2 适配器；
未把未公开的上游 shared 包当作已安装 npm SDK，也没有新增第二种扫码协议。

## 用户已确认规则与待确认项

后续用户确认覆盖最初的工程默认值：

| 项目 | 当前采用的口径 |
| --- | --- |
| 资金模式 | `revenue_share`：按实际合格收入分成，没有固定活动总预算，不要求预存现金或为新成员预留现金；四项人数／收入上限可为0。兼容旧配置的 `budget_capped` 仍保留 |
| 活动区间 | 北京时间2026-11-01 00:00至2027-02-01 00:00，含开始、不含结束；对应 UTC `2026-10-31T16:00:00Z` 至 `2027-01-31T16:00:00Z` |
| 推荐与资格 | 直接一层邀请、分段10%／20%／30%，等级30及3个 UTC 有效游玩日；7日候选、注册后24小时确认且在首次付费前完成归因，月卡服务按赛季交集及覆盖上限计算 |
| 计价与付款 | USD 记账、USDC 支付；先在 Sui Testnet 验收，再按独立生产配置开启 Mainnet。币种必须绑定具体 coin type、chain ID、精度及固定换算快照 |
| 收款地址 | 本次登录实际验证的钱包；服务端派生，不接受网页任填地址，也不能选用同账号的另一历史钱包替代 |
| 50人算例 | 50名首档合格成员各产生10 USD收入，总佣金为50 USD；采用1:1 USD／USDC结算口径时，总应返50 USDC，不是每人50 USDC |
| 准备金 | 用户已明确核账后一次发满50 USDC；当前活动草案 reserveBps=0，不扣留10%。已发生退款和不合格收入仍核减 |

未付费注册、点击及活跃只属于效果统计，不形成现金债务。实际商品价格、税费证据、
条款版本／公开条款、地区、身份／税务政策、最低付款额、Gas承担方式以及独立审批
仍须落实到明确配置，不能用算例、测试夹具或空白字段代替。

默认仍不自动创建活动或发送付款。`0022` 引入配置哈希绑定的上线 manifest、operations
提案与不同 finance 审核人，以及数据库开关门禁；`0023` 支持上述无固定预算分成模式。
Testnet 可在手机／支付平台／链上验收字段尚空白时先验证 policy 和操作流程，但不因此
获得真实验收状态。Production 需要真实证据与独立审核；Mainnet 付款还必须同时通过
服务器及执行 CLI 的显式主网开关。

## 接口

浏览器调用同源 `/api/promotions` BFF：

| 路由 | 行为 |
| --- | --- |
| `POST /auth/challenge`、`POST /auth/poll` | 创建及轮询浏览器绑定的钱包挑战 |
| `POST /auth/password` | 先验证原账号，用于双控制绑定；不注册账号或保存密码 |
| `POST /auth/logout` | 撤销原身份会话并清理 Cookie |
| `GET /bootstrap` | 活动状态、本人的角色、身份、看板、资格与权限 |
| `GET /public`、`GET /invite?code=...` | 公共条款状态与邀请关系披露 |
| `POST /click` | 记录匿名候选点击，临时标识只保存在受保护 Cookie |
| `POST /candidate`、`POST /confirm` | 候选及新人最终确认；账号和来源由服务端核验 |
| `POST /apply` | 以本人真实角色申请团长 |
| `GET /members`、`GET /settlements` | 仅查询本团长的脱敏成员和结算 |
| `GET/POST /payout-address` | 从本次登录验证的钱包派生收款设置及变更冷却，不接受任意地址金额 |
| `POST /appeals` | 提交有审计记录的申诉 |
| `GET/POST /admin` | 按数据库操作员角色审核、核账、上线 policy、结算与付款意图；不接受浏览器自报权限 |
| `GET /export?settlementId=...` | finance／auditor 导出结算规范 JSON 与哈希，网页按原始字节核验后下载 |

认证写请求使用 `x-promotion-csrf`，业务写请求另需 `Idempotency-Key`。
重复幂等键必须携带同一操作和相同输入；冲突会被拒绝。分页采用 offset／limit，
最大100条。所有私有响应禁止缓存。

Rust 同类路由位于 `/v1/promotions`；仅受验证的 `IdentitySession` 决定调用主体。
`POST /apply` 还要求本账号具有未撤销的钱包凭据；只完成原密码验证时不能绕过网页
直接申请团长。操作员权限仍由数据库授予，不受浏览器提交的角色影响。
可信服务端 `POST /v1/promotions/source-events` 不经浏览器 BFF 开放。生产游戏事实须与
实际提交的来源 inbox 匹配；真实月卡收入由 worker 直接读取权威账号的批次与权益，
不接受浏览器金额。HMAC 签名绑定用途、方法、路径、毫秒时间戳与原始请求体，并限定
60秒时差；数据库唯一键再次防止重放。`/v1/promotions/payout-worker` 使用另一独立密钥
及服务器配置的服务账号，处理已审核交易的预留、提交及结果记录。

## 资金与资格来源

前轮只具备充值／月卡收据和显式 sandbox 夹具适配器，缺少完整实时批次与游戏事实；
本轮已增加以下实际代码路径。旧 sandbox 入口仍禁止
进入 production，它也不能替代真实支付平台验收。

- 游戏来源：升级、共享 Zone 的权威击杀、普通及周期任务完成，在原 SourceOfTruth
  存档 CAS 内追加不可变 inbox。worker 读取确切来源再写推广事实并确认投递；跨 UTC
  午夜保存仍使用原事件时间。历史等级、旧完成标记及快照不会反推不存在的事件日期。
  走路、在线时间、网页停留及客户端心跳不计有效游玩日。
- 点券来源：`BillingLedger.provenance` 与余额、充值应用、商城／月卡购买共同提交。
  新的已验证充值产生 paid lot；历史余额为 unknown；普通消费与赠礼同样按 FIFO 消耗，
  不会把用于其他商品的现金批次留给下一次月卡重复计佣。礼物及付款人／受益账号不一致
  的月卡排除，原有付费订单不能事后把历史余额重标为现金。
- 月卡收入：只有实际由该账号兑付的 UID 及真实服务期可导入，绑定消费的多个 lot、
  原订单、税、固定 FX 与证据。充值成功、扣点、开卡不会分别重复计佣；同一 UID 在
  同一环境跨赛季只入账一次。TEST 订单无法进入 production 收入。
- 支付证明：沿用 Stripe 验签和服务端订单重查，追加实收、税、折扣及支付来源记录。
  USD 使用固定整数换算（1美分=10,000 microUSD）；缺税、未批准的外币换算或来源
  不明均待核查。支付渠道手续费按实施方案列为官方成本，不暗中从佣金基数扣除。
- 退款及争议：重新查询 provider 的累计退款及当前争议状态，沿原 lot 成本冲正。
  未完备的回调、争议、金额冲突或不能证明是否重叠的退款／拒付会建立 active hold，
  阻断旧收入继续结算；补挂邀请前已发生的付款进入归因审核。暂挂不会清空既有资格
  时间，复核恢复后保留原排序；真实全额退款仍撤销资格。

迁移对应：`0018` 多 lot 明细与来源快照，`0019` 权威游戏来源，`0020` 付款意图与交易
日志，`0021` 收入核账 hold，`0022` 上线双审，`0023` 实际收入分成。它们不删除旧金融记录。

## 配置

仅示意配置名称；密钥值不进入仓库、页面、日志或构建产物：

| 名称 | 用途 |
| --- | --- |
| `MIR2_GATEWAY_HTTP_URL` | BFF 显式可信网关，禁止由浏览器选择或跟随重定向 |
| `MIR2_PROMOTION_GATEWAY_ALLOWED_ORIGINS` | 可选网关 origin 白名单 |
| `MIR2_PROMOTION_SESSION_SECRET` | 独立、至少32字符的 Cookie 加密密钥，不与 passkey 共用 |
| `MIR2_PROMOTION_CAMPAIGN_ID` | 明确选择且与环境匹配的活动，无默认 demo |
| `MIR2_PROMOTION_ENVIRONMENT` | 网关业务环境，仅 development、test、production |
| `MIR2_PROMOTION_SOURCE_SECRET` | 可信来源事件 HMAC 密钥，不提供给前端 |
| `MIR2_ACCOUNT_STORE_DATABASE_URL` | 既有权威账号 PostgreSQL |
| `MIR2_PROMOTION_PAYOUT_WORKER_SECRET`、`MIR2_PROMOTION_PAYOUT_WORKER_ACCOUNT` | 独立付款服务密钥及受控服务账号，不复用来源密钥或人工提交者 |
| `MIR2_PROMOTION_ALLOW_MAINNET_PAYOUT=true` | 网关主网执行门禁；不能代替活动与交易审批 |
| `MIR2_PROMOTION_PAYOUT_MAINNET_APPROVED=1` | 付款 CLI 的额外主网执行开关 |
| 既有 `MIR2_STRIPE_*`、月卡价格配置 | 真实 provider／TEST 与 LIVE 隔离、订单与月卡商品；不得用示例密钥代替 |
| 既有 `MIR2_DUBHE_*`、passkey、identity 配置 | 沿用登录协议与身份验证，不另生成生产身份体系 |

`promotion_operator_roles` 为受控数据库授予的 operations、risk、finance、
payoutSubmit、payoutReview、auditor 权限。网页没有给自己赋权的接口。
活动配置者与审核者、结算生成者与审核者以及付款提交者与复核者必须分离。

开发网页可单独启动，不执行游戏资源构建或 Docker：

```powershell
cd mir2-web3/apps/web
npm run dev:promotions -- --hostname 127.0.0.1 --port 7359
# 浏览器打开 http://127.0.0.1:7359/promotions
npm run test:promotions
```

先使用项目锁定版本安装依赖（Node22，`npm ci`）。当前机器访问真实 relay 需使用已验证
的 Node24.19.0 代理环境（`NODE_USE_ENV_PROXY=1`，并将回环地址加入 `NO_PROXY`）；
Node22 的同一请求曾超时，不能只因变量已设置就声称代理生效。真实 relay 创建201、
轮询200及钱包批准页200已核实，没有关闭 TLS 校验或更改 Cloudflare。

如未配置网关、Cookie密钥及活动，
页面明确显示未配置或服务不可用；这时可检查页面与语言，不能伪造账号或收入成功。
网关沿用现有原生服务和 PostgreSQL 配置，不用上述命令修改已有测试服。
密码请求使用原有账号／设备／socket peer 限流；BFF后所有用户共享服务器peer，
真实流量接入前需配置可信入口策略，不能直接信任浏览器提供的转发IP头。

## 可复算与恢复

金额内部使用整数 microUSD，费率使用基点。纯函数以源事件时间及不可变账号排序，
先逐成员算高精度分子，最后按团长赛季汇总舍入美分。跨季服务只计算UTC区间交集，
重叠覆盖不重复，未知来源不计入。封账保留输入、算法版本、结果及哈希。

退款追加冲正，保留旧记录。封账前可按资格重排；封账后只按原成员原费率冲回，
不重排其他正常成员。审计与财务分录禁止业务更新／删除。

内部 outbox consumer 使用事务与 `FOR UPDATE SKIP LOCKED` 领取有限任务，按活动
合并资格刷新、暂计快照和预算对账。完成投影后才标记完成；失败保留次数、错误码及
下次重试时间；进程退出前未提交的领取会回滚。后台 `/admin` 的 `queueHealth` 提供
积压、失败、最老等待、最后处理及外部适配未接通计数。

worker 必须由运维明确调用，本轮没有安装服务或自动启用周期任务：

```powershell
# 先在受保护环境配置上述 PostgreSQL 与推广环境变量
cargo run --locked -p mir2-simulation --example promotion_worker -- --once --limit 50
# 确需持续运行时再显式选择 --poll-ms 30000
```

该 worker 现在依次投递已提交游戏事件、核账权威月卡批次收入、恢复待处理退款，再做
内部资格／佣金投影。来源导入使用独立数据库 session advisory lock，避免跨进程重复
冲正；金额变化先持久化 hold 和目标，崩溃后重试缺失差额。外部告警／消息投递仍未
接通，不能把 outbox 完成当作通知已发送。数据库拥有者仍能变更 DDL，业务触发器
不能替代最小权限与审计备份。

付款适配器已实现 Sui gRPC 的精确整数字段、网络／coin type／Gas／纪元、真实交易签名
验证、PG 先记录签名字节与 digest 再首次提交、未知结果只查询同一 digest 的恢复流程。
freeze、review、reserve、submit 均重新检查来源 hold、退款、收款版本及受限团长；
8并发预留只允许1个首次发送者。CLI `apps/web/scripts/promotion-payout.mjs` 默认只读
`probe`，另提供 `prepare`、`verify`、显式 `execute` 与 `reconcile`；不接收私钥参数，
不自动签名。签名由外部钱包或受控的隔离测试签名器提供；真实交易仍需可用 Gas、
相应币种、有效签名和审核后的具体交易，登录签名不能代替交易签名。

结算导出返回 `snapshotCanonicalJson` 字符串，不把其中的64位金额交给浏览器浮点解析。
规范化时递归排序对象键，保留十进制整数；服务器和网页下载前都检查 SHA-256。
当前导出沿用 BFF 1MiB 响应上限，超过上限会明确失败，不截断文件。

离线重算工具不连接数据库或钱包；先核对导出原字节的 SHA，再使用对应算法版本
重算并比对整个结果：

```powershell
cd mir2-web3
cargo run --locked -p mir2-simulation --example promotion_recalculate -- C:/path/to/export.json
```

前轮非生产样例 `promotion-settlement-fixture.json` 使用合成账号及
持久化充值／权益夹具，没有真实债务；验算通过不代表该文件已获得财务付款授权。

关闭网页入口或回退应用版本时，保留新增推广表、审计、结算及来源记录；不得通过
删除推广表来回退，也不得清空原账号和角色。停止新的来源投递前保存游标，修复后按
原事件ID补拉。旧游戏备份不得回退已有付费批次、月卡历史或对应余额；此类恢复返回
`billingFinancialRestoreRequiresReconciliation`。未知付款状态必须查询原交易，不能
重新生成转账；本轮没有发送交易。

## 验证记录

本轮已完成的检查均为原生进程，没有 Docker。下表区分源码／隔离数据库验证与真实
外部验收；不同测试集合可能重叠，不重复汇总。

| 范围 | 实际结果 |
| --- | --- |
| 权威游戏来源 | 10条纯单元、1项真实存档生产者测试、1项隔离原生 PG 测试通过。覆盖同事务回滚／提交、确切击杀收据、跨 UTC 午夜、并发投递、确认前崩溃重放、删角／封禁后历史、篡改及 TRUNCATE 拒绝 |
| 付费批次与退款 | 15条新增 Rust 单元通过，覆盖 paid／gift／unknown、普通消费先消耗、多个 lot、税／FX／环境、乱序累计退款、争议、回调中断暂挂、旧备份金融保护；43条既有商城单元／运行时回归通过 |
| 真实游戏金流 API／PG | 1项原生 PG 综合场景通过：实际 prepare／confirm／apply／buy／activate 与重启；两个充值 lot 分摊出9 USD月卡收入，首单50%退款只冲回1.50 USD；并发／重放、早付归因审核、历史 unknown、争议阻断结算、测试款不进入生产均通过。provider 输入明确为夹具，不是外部 Stripe 扣款 |
| 无固定预算分成 | 2项测试通过：50×10 USD首档收入生成50 USD总应付、零准备金的纯算例；原生 PG 全流程配置 `revenue_share`、预算和四项上限全0，接纳2名真实账号、实际购买兑付月卡、收入20 USD产生2 USD佣金、现金预留0、暂挂保留资格时间、恢复及全额退款均通过 |
| Sui 付款适配器 | 20项 Node 测试通过，包括本地真实 Ed25519 交易签名、精确整数、网络／币种、结果未知时只查原 digest 等；RPC 故障部分为 mock，不能当作链上付款验收 |
| 付款日志／PG | 隔离 PG 测试通过：意图／签名字节不可变、独立角色与 SHA 审核、8并发仅1次首次发送认领、未知结果恢复、最终状态不倒退、地址版本／最低额／受限团长／来源 hold，以及预留后、发送前的新退款阻断 |
| 上线门禁／PG | 隔离 PG 测试通过：未审开关和直接 SQL 开启被拒绝、独立复核、配置与 manifest 哈希绑定、材料不可改删、coin／network／FX 与主网部署标志检查；生产路径 manifest 为合成夹具，没有形成真实上线批准 |
| 网页组合测试 | 78项全部通过（含固定公开 origin 的代理／CSRF 边界、只签名交接及新增九语钱包 UI），含 BFF、钱包相邻、界面、审核及付款逻辑；其中已包含上述20项 Node 付款测试，不能重复相加。审核页面英／繁中／阿拉伯语在1280px与390px夹具截图无溢出、无页面异常 |
| 真实 relay 连通 | Node24代理环境下 Dubhe Connect v2 创建201、轮询200、批准页200；challenge／envelope匹配；用户已在手机真实签名，实际页面登录态和原生 PG 的 sui_wallet credential／session均已核验，无夹具代签 |
| 真实 Sui Testnet 小额付款 | 0.01 testUSDC最终成功，digest `5mrxwBAXXu8ntDmmbSnrmaHKju1wonoLCmJpqyNpgriM`、checkpoint `400277484`；只发送1次。PG原始签名字节一致，payout／attempt均confirmed；付款20→19.99、真实登录收款钱包0→0.01，Gas为0.002322532 testSUI。协调者独立重查真实RPC及PG通过；本次由用户指定的本机隔离测试key签名，不算手机交易签名或真实赛季收入验收 |
| 本轮完整网关／HTTP／相邻集成 | 最终冻结源码 Gateway 构建通过；真实 HTTP 14阶段／90请求通过，含当前登录钱包绑定和 worker 独立密钥／路径隔离；现有充值26、月卡10、赠礼15项通过，1项专用 PG gift 未运行。真实 Gateway SourceOfTruth 击杀提交及 Mirror 相邻各1项 PG通过；Stripe provider 本地 HTTP 10项通过（不是外部平台付款） |

本轮详细证据统一保存在 [验收证据目录](C:/Users/Administrator/AppData/Local/Temp/mir2-promotion-live-20261029/)：
游戏来源为 `game-source-evidence.json`，金流与分成为 `payment/`，Sui执行及上线门禁为
`payout/`，网页为 `web-tests-final.log` 与审核截图，真实 relay 为
`relay-connectivity-20261029.json`。私有配置及 synthetic Testnet 私钥不进入公开证据。

前轮开发基线证据仍保留于同级 `mir2-promotion-20261029/` 的原 manifest；其中曾完成
20条推广算术测试、12场景69次真实 HTTP 请求、锁定依赖 projection 构建及旧的预算模式
检查。这些是历史快照，不能证明本轮新增来源、付款端点或生产配置已完成完整网关联调。
旧样例81美分应付采用当时10%准备金，不适用于当前用户已确认的零扣留、核账全额支付规则。

真实 HTTP 复跑脚本需显式传入仓库外的私有配置和证据目录：

```powershell
node mir2-web3/apps/web/scripts/test-promotion-gateway-http.mjs --config C:/private/promotion-test.json --evidence-dir C:/qa/promotion-http
```

私有配置字段：`gatewayUrl`、`databaseUrl`、`psqlPath`、`passkeyAuthSecret`、
`sourceSecret`、`environment`（development／test）。脚本限制回环地址与推广专用空库，
没有参数时不执行，也不删除旧数据。Dubhe relay 明确使用 SDK 签名夹具；网关、
身份会话、账号及数据库为实际代码，不以此代替钱包手机或外部支付平台验收。

## 仍待完成的真实验收与配置

- 手机实签已通过：用户在真实 Dubhe Wallet 完成批准，实际页面及原生 PG credential／session核对通过；证据 mobile-signature-acceptance.json。登录签名只证明身份，不能授权付款。
- Stripe 沙盒：当前机器进程尚无可用的 provider 沙盒配置／凭据，未发起真实外部沙盒
  支付或退款。原生 PG 与本地 HTTP provider 夹具已验证代码边界，仍须以实际沙盒订单
  补齐平台实收、税、回调、月卡分摊及退款核账证据。
- 跨分支支付来源：用户指定的“确认多人战斗与动态分区”会话正在独立验收 Sui USDC 充值。只读核对其 `sui_usdc_billing.rs` 后确认：该分支的已验证链上 receipt 尚未写入本轮 FIFO provenance；双方 `BillingLedger` 分别包含 `sui_usdc_orders` 与 `provenance`，均拒绝未知字段，不能直接共享账号数据。需要统一 schema，并在充值入账事务中追加带 provider／环境的来源 lot，月卡消费时再分摊收入。另一实现目前强制 Testnet／Test，不能计入生产收入。本轮未修改或合并那个会话的代码。
- Sui Testnet技术付款已通过：用户指向另一会话后，已只读定位 `0x960b…97a33` 的本机隔离测试签名器；原始key只在内存读取，没有复制到本仓库、网页或日志。固定交易SHA e68e3d…60776／digest `5mrxwBAXXu8ntDmmbSnrmaHKju1wonoLCmJpqyNpgriM` 先持久保存签名，再由PG预留及发送标记后提交。checkpoint `400277484`成功，只有1次execute，付款钱包20→19.99 testUSDC，真实手机登录钱包0→0.01，Gas为0.002322532 testSUI。共享测试钱包锁覆盖签名至最终核账，已释放；未操作另一会话的订单或进程。证据位于本轮临时目录 `payout/chain-acceptance-final-receipt.json`、`chain-acceptance-execution-order.json`、`chain-acceptance-root-audit.json`，协调者已独立重查RPC及PG。此为技术夹具／synthetic审批，不计实际赛季收入、真人财务审批或手机付款交易签名。
- 准备金：用户已确认核账后一次付足，当前活动草案使用0准备金；总应计佣金、退款与已付须分别列示，
  不再采用原方案10%的扣留比例。
- 上线：上述真实验收、明确条款／地区／身份税务／最低额／币种换算等配置完成后，
  通过 operations 提案及独立 finance 复核，再按授权开启相应开关；正式主网付款还需
  显式 mainnet flag。Testnet policy 演练可先行，生产验收证据不能用演练材料冒充。

当前用户已确认的可审阅活动草案见 [测试网赛季配置](../config/promotion-season-20261101.test.json)：北京时间2026-11-01至2027-02-01、实际收入分成、零固定预算／名额预留、零准备金。未提供的最小付款门槛、地区／条款／税务口径保持未配置；草案文件本身不构成审批或已付款证据。

部署兼容性：写入 billing.provenance 后，旧版 deny_unknown_fields 的 Source／Gateway不能读取新账本；需协调升级所有共享账号 PG 的读写进程，或先部署兼容 reader 的回移版本。不得删除已验证金融字段以回退旧程序。本轮没有部署或修改其他会话负责的 Windows 自动更新。

本地运行：公开测试入口转至7357的独立推广站构建，经7374限制路径代理和Cloudflare Tunnel访问；7359开发网页已停止。7361为已更新测试报名功能的Gateway，推广worker每10秒归档实际来源；7371为独立技术付款验收Gateway。7373只签名页面已因15分钟TTL到期退出，没有保存签名。原手机账号和钱包凭据均保留；公网域名不同，需要重新扫码登录。独立持久技术账本不得随临时测试schema清理，尤其已有signed attempt／unknown状态时。

## 七天公开测试赛季（2026-10-29）

用户要求让他人试玩后，新增独立活动 `numeron-playtest-20261029`，北京时间
2026-10-29 04:35:38 至 2026-11-05 04:35:38。已用实际 `PromotionStore.admin`
配置、测试规则复核并开启招募和计佣；重跑配置工具保持幂等。正式三个月活动仍是原草案。
配置见 [七天测试配置](../config/promotion-playtest-20261029.test.json)，工具为
`apps/simulation/examples/promotion_playtest.rs`。测试配置由两个无已知登录密码的本机系统账号
执行，记录明确为自动测试审批，不是独立真人财审；没有开启付款。

分享入口：<https://expand-marilyn-bet-betty.trycloudflare.com/promotions>。
它通过临时 Cloudflare Tunnel 访问本机，只在本机服务和隧道持续运行时有效，重建隧道会换域名。
公网代理只放行推广页、指定推广 BFF 和必要资源；管理、游戏 API、source／payout-worker、
源码、环境文件和开发 WebSocket 均不开放。公共域名固定用于来源校验和加密 Cookie，不信任
浏览器传入的 forwarded host／身份；换域名需同步服务配置，不能直接转发任意站点。

为便于新钱包试用，规则显式设置 `testWalletEnrollment=true`。只在 `environment=test`、
规则已复核、招募开放且位于活动时间内，验证未撤销的钱包凭据后允许 `characterIndex=-1`
报名并立即取得邀请码；这个 -1 是钱包展示身份，不创建游戏角色或等级。非测试环境严格拒绝。
旧规则默认关闭此功能，false 不增加序列化字段，旧审批散列保持兼容。
测试资格门槛为1级、1个UTC有效游玩日，正式草案仍是30级／3日；仅登录或邀请不产生佣金资格。

试玩顺序：打开上述 HTTPS 地址→Dubhe Wallet 测试网扫码登录→钱包体验报名并确认测试条款→
复制本人邀请链接→另一新钱包先从邀请链接进入再登录并确认绑定→团长查看成员。
登录无需付款或领取测试币；不要把旧 `127.0.0.1:7373` 签名页发给朋友。
当前可验收登录、报名、分享、归因及统计；另一分支USDC充值来源与自动赛季付款仍待接通，
页面不会填充虚假的收入，测试资产没有真实现金价值。

本轮证据集中在临时 `mir2-promotion-live-20261029/playtest/`：新增原生PG／规则兼容2项、
UI与审核相邻21项、代理与既有网页组合96项通过；真实公网18次HTTP完成两临时钱包报名和
成员归因，PG确认没有角色、游戏事实、收入或付款。此处授权／Cookie是显式夹具，不冒充手机。
独立真实relay测试确认公网挑战200、轮询pending200及钱包批准页200；没有代替用户批准。
正式草案和原手机账号快照均保持相等。详细记录见 `PUBLIC-BFF-FIXTURE-E2E.json`、
`PUBLIC-REAL-RELAY.json`、`share-proxy-assets.json`，最后一项位于专题临时目录根。

公开入口最终使用只包含推广模块的生产构建，保持测试赛季／Testnet语义：25份业务源文件SHA
与当前仓库一致，新增最小根布局以移除游戏PWA和缓存注册；Next16.4.1／Node24，严格TS及
Webpack生产构建通过，运行于本机7357。复用C盘已存在的锁定依赖，没有修改依赖或打包密钥；
原E盘开发依赖实际为Next16.2.11，跨盘junction构建失败日志保留，未将失败写作成功。
首屏JS由4,425,312减至760,969解码字节，减少82.8%，HMR／devtools引用归零；最终10项页面
资源和匿名bootstrap全部200，非推广／管理／source／worker路径404。生产构建版本再次完成
18次实际公网HTTP邀请闭环，授权仍为隔离夹具；见 `PUBLIC-BFF-PRODUCTION-E2E.json`。
浏览器已实际显示测试报名入口、正确日期及开启状态，无控制台错误，画面为 `public-site.png`；
这些不代替新域名下用户本人的手机批准。构建清单位于 `playtest/site/`，运行摘要为
`playtest/runtime.json`；启动脚本加载本机私有配置，密钥不进入上述公开证据。

## 原官网 Vercel 发布（2026-10-29）

按用户要求，推广页已发布到原官网现有 Vercel 项目 `Obelisk Labs / mir2-web3-web`，
分享地址改为 <https://mir2.obelisk.build/promotions>。它使用官网已有 Cloudflare 代理，
本轮未改 DNS、Worker 路由或原游戏环境变量，没有启动 Docker，也没有推进 Windows 自动更新。
旧临时公网代理于 UTC 04:11:56 冻结，以上临时隧道运行记录仅作为历史证据。

网站发布分支为 `codex/promotion-vercel-20261029`，实际部署源码
`76cef9c57876afb363bf75b61afc075169538b8f`，Vercel 部署
`dpl_48nA9MP31zPZXMHxgmfciatquZyd`。以原线上 `119553ff6` 为基线，仅移入推广模块、
独立服务端鉴权上下文和必要依赖；11项原首页／布局／PWA／构建／资源配置逐字节保留。
完整网站生产构建与全站 TypeScript 检查通过，鉴权相邻92项、界面／审核21项通过。
GitHub run `40818351183` 的 promote 请求已生效，但 CI 因 Vercel 成功返回空正文后调用
JSON 解析而报告失败；已通过原 Vercel alias 解析至上述部署，以及官网真实 HTTP 验证确认
切换成功，没有重复发起发布。
随后仅修复 CI 空响应解析，提交 `5b7edeb15` 已推送同一发布分支；7项离线响应测试通过，
其余守卫逐字节未变，没有重新部署。当前网站仍对应上述 `76cef9c57`。

推广后端独立部署于原测试服务器，源码
`e2796cf721c3a8e2d8d182dba94c74af99012508`，使用原生 Gateway、worker、
独立回环 Redis 6382 和专用 PostgreSQL 库 `mir2_promotion_playtest`。
CI 锁定依赖构建产物及4份载荷 SHA 均已核验。冻结入口后 UTC 04:16:20 导出一致性快照，
启动新服务前76张表的数量和内容摘要全部相等，保留7个测试账号、5份凭据、5份钱包映射及
原手机身份恢复材料。技术付款验收账本留在原处，没有复制为赛季债务。
原游戏119个账号、35个角色、35份存档，以及原3个服务 PID 前后相等，旧服务健康检查200。

UTC 04:34:13 在官网完成14项实际 HTTP／relay 检查：原首页、推广页和匿名活动200；
错误邀请码404，未登录私有接口拒绝，跨来源403、浏览器 bearer400；真实 Dubhe relay
挑战与轮询均200/pending，网络明确为 Testnet，来源固定为官网，加密挑战 Cookie 的保护
属性通过。网页截图已实际确认七天日期、招募／计佣开启、付款关闭及完整页面排版。
这些检查没有代签、创建钱包账号或转账，不能替代新官网域名下用户本人的手机批准。

测试者打开官网推广页后，重新扫码登录，再按页面测试条款自行报名、复制邀请链接，
另一新钱包从邀请链接进入并确认绑定。活动仍为 `numeron-playtest-20261029`，
截至北京时间2026-11-05 04:35:38；正式三个月赛季保持草案，付款开关保持关闭。
新服务没有自动接入旧游戏进程的真实事件；外部 Stripe、另一分支 Sui 充值来源和
自动赛季付款仍须分别集成验收，不得把本次网站发布描述为完整真实收入返利上线。

证据集中于本专题临时目录的 `vercel-release/RELEASE-RECEIPT.json`、
`PUBLIC-RELEASE-HTTP.json`、`public-vercel-site.png`，以及
`deployment/REMOTE-DEPLOYMENT-RECEIPT.json`、`REMOTE-PUBLIC-HTTP-VERIFICATION.json`、
`SOURCE-SNAPSHOT-RECEIPT.json`。回退网站时可恢复此前部署
`dpl_3fnCbybhxwi7AXawCLjLkRTZkLDo`；保留新数据库、来源游标、审计和付款材料，
不得靠回退旧库或删除金融字段恢复旧界面。
