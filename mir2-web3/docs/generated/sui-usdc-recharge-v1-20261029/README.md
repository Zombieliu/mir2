# Sui USDC 充值验收包 — 2026-10-29

本专题只记录 Sui 原生 USDC → 游戏点券充值。已有 Stripe 和 Dubhe 金币求购保持各自边界；没有主网开放、生产收费或远程测试服发布。

**当前：真实 Circle Testnet USDC 付款、数据库/Windows 点券到账、v4 重启重登及关闭新充值后的恢复均已验证。** 操作使用显式隔离 QA Wallet，普通钱包扩展的真人最终接受仍开放。下方 Mock 图与真实交易及实际 Windows 截图分开记录。

## 已执行检查

| 检查 | 结果 | 证据 |
| --- | --- | --- |
| 模拟服 USDC 状态机 | 21/21 | [最终日志](rust/simulation-usdc-final.log) |
| 模拟服 Billing 过滤集 | 24/24，包含上述 21，不能重复累加 | [日志](rust/simulation-billing-final.log) |
| 原个人求购相邻回归 / schema | 54/54 / 3/3 | [采购](rust/simulation-procurement-adjacent.log)、[schema](rust/simulation-schema-adjacent.log) |
| Gateway USDC / 恢复 / 环境门禁 | v3 13/13；v4 新增过期边界后 15/15 | [v3](rust/gateway-usdc-final-v3.log)、[最终 v4](rust/gateway-usdc-final-v4.log) |
| Gateway 相邻 Billing / Stripe | 21/21 / 4/4 | [Billing](rust/gateway-billing-adjacent.log)、[Stripe](rust/gateway-stripe-adjacent.log) |
| SQL 0019 | PGlite PostgreSQL 18.6 WASM 31/31 | [结果](sql/result.json)、[可重跑脚本](sql/test-migration.mjs)、[固定依赖](sql/package-lock.json) |
| Windows 原生充值逻辑 | 14 passed，逻辑命令有原 1 项 ignored GPU 用例 | [逻辑日志](native-mock/billing-logic.log) |
| 独立 Windows GPU 用例 | 1/1，10 张状态图，缺字/溢出/越界均为 0 | [执行日志](native-mock/billing-gpu.log)、[完整布局报告](native-mock/gpu/report.json) |
| Windows host / URL / 作用域 | 7/7 | [host 日志](native-mock/windows-host.log) |
| 浏览器支付页 | 18/18，Mock Gateway + Mock Wallet，无密钥/无链交易 | [脱敏报告](browser-mock/report.json) |
| 真实 Windows 交互 | 创建、0→100 点券、重复刷新、普通商店、v4 重登及停售恢复 | [实际截图与关联](windows-live/screenshot-associations.json) |
| 链上 SDK/Move | Node 22/22、Move 14/14 | [最终 Node 22](../../../onchain/usdc-recharge/validation/node-test-lifetime-gas.log)、[链上验证记录](../../../onchain/usdc-recharge/validation/README.md) |
| 最终 Gateway / Windows 构建 | 均成功；仅本机测试产物，不是发布包 | [二进制元数据](binaries.json)、[最终 Gateway v4](rust/gateway-build-final-v4.log)、[付款 Gateway v3](rust/gateway-build-final-v3.log)、[Windows 构建](native-mock/windows-build.log) |

PGlite 检查证明本迁移的唯一键、触发器、回执及创建栅栏与账号更新的事务回滚行为；**不证明原生 PostgreSQL 多连接并发或多机恢复已通过**。Rust 并发测试也不能替代两个实际数据库连接的冲突验收。

## UI 图像来源

`native-mock/gpu/` 的 10 张 PNG 使用实际 Windows 生产渲染器在离线状态 fixture 中生成，覆盖英文/繁体的 Stripe、USDC 准备、确认、过期/复核和停售恢复页面。没有真实钱包签名和链上付款。GPU 用例已独立执行，未把逻辑过滤命令中的 ignored 当作验证。

`browser-mock/` 使用隔离 headless Chrome、Mock Gateway 和显式标记的 `OFFLINE QA Wallet (no keys)`。公开报告保留 18 项结果与观察，删去了详细 cases、请求原文和签名；没有复制浏览器 harness 或能力凭证。

| 示例 | 文件 |
| --- | --- |
| 原生 USDC 页面（繁体） | [准备页](native-mock/gpu/zh-TW-usdc-preparing-1024x768.png)、[确认页](native-mock/gpu/zh-TW-usdc-confirmation-1024x768.png) |
| 原生停止新销售后的旧订单恢复 | [状态图](native-mock/gpu/zh-TW-usdc-sales-disabled-1024x768.png) |
| 浏览器 Mock：链上已付、游戏待到账 | [桌面状态图](browser-mock/desktop-chain-paid-game-pending.png) |
| 浏览器 Mock：游戏已到账 | [移动状态图](browser-mock/mobile-game-delivered.png) |

## 实际验收状态与限制

根协调器通过真实 Windows 创建订单 `usdc_066575bf24e8ede6000e73d8d0fc2aaa9bdb475aa3e44d7faa72f1bff215fd7e`，在登记的浏览器中显式选择 `Isolated QA Wallet (real Testnet)`、签消息并确认付款。独立官方全节点+游戏数据库[最终审计](live-real/final-recharge-audit.json)的 12 项检查全部为 true；[全局唯一性审计](live-real/global-uniqueness-audit.json)确认没有重复发票或付款回执。

| 实际结果 | 证据 |
| --- | --- |
| 发票 `0xfdb531141bf73b1affb010b1ef3fcf9dcc6b16777ed7027fde481dbb4f27b999` | 创建 `6Kmz6ohmUNmpEipaZetWuR5HvZnTy93EkfhnyeQUKZvV`，checkpoint 400291251，Gas 4,815,356 MIST |
| 付款 1 原生测试 USDC | `E2yqrhVNuVPCWcvJkmW93fbXfHAfRLkqXyTqiTcdA4SJ`，checkpoint 400291862，Gas 2,322,914 MIST |
| 买家/商户 USDC | 19.99→18.99 / 0→1，见[最终付款回执](live-real/payment-finalized-receipt.json) |
| 游戏点券 | 唯一 `Applied`，0→100，来源 Test；实际 Windows 展示与重登后余额均为 100 |
| 成功单两笔 Gas 合计 | **7,138,270 MIST = 0.007138270 测试 SUI**；设置和旧尝试另列，不含服务器成本 |

付款由 Gateway **v3** 处理。v4 只修复已确认未付发票的过期公开状态，持久状态和恢复不变；后续 v4 重登不能改记为 v4 产生付款。普通钱包扩展/真人 Accepted、原生 PostgreSQL、多机和生产收费仍开放。

## 实际 Windows 到账、重启与重登

以下六张 JPEG 来自实际 Windows 目标窗口的 Computer Use 截图，带原始时间/窗口 sidecar，已逐张视检；不是 Web 截图或离线渲染 fixture。截图和数据库快照不是同一原子采样，按[关联说明](windows-live/screenshot-associations.json)、订单和时间核对。

| 阶段 | 实际 Windows 证据 |
| --- | --- |
| 成功订单付款前，点券 0、pending 0 | [05](windows-live/screenshots/05-native-final-order-zero-credit.jpg) |
| v3 真实付款后点券 100、pending 0、订单已到账 | [06](windows-live/screenshots/06-native-real-100-credits-delivered.jpg) |
| 再次刷新仍为 100 | [07](windows-live/screenshots/07-native-repeat-refresh-100.jpg) |
| 普通 GameShop 显示点券 100、金币 10,000 | [08](windows-live/screenshots/08-native-shop-100-credits-gold-unchanged.jpg) |
| v4 服务重启、正常重登后的 GameShop | [09](windows-live/screenshots/09-native-v4-relogin-shop-100.jpg) |
| 新充值关闭后：点券 100，pending 0，两个旧单过期，成功单已到账 | [10](windows-live/screenshots/10-native-v4-relogin-disabled-sales-delivered.jpg) |

v3 的[重复付款通知核验](live-real/public-duplicate-delivery-verification.json)与 v4 重启/停售后的[同 digest 两次通知及查询](live-real/public-v4-disabled-sales-recovery-verification.json)均返回 200/Applied，点券 100、金币 10,000、唯一回执及唯一到账订单不变。只重放通知和查询，没有再次签名或广播付款。

最终[正常重登快照](windows-live/evidence/v4-final-relogin-confirmed.json)为 schema13、点券 100、金币 10,000；[停售重启快照](windows-live/evidence/v4-restart-sales-disabled.json)保留同一付款与到账回执。最终运行环境为直连 `ws://127.0.0.1:17930/ws`，`MIR2_SUI_USDC_ENABLED=0`，原 QA 浏览器代理已停止；既有冻结配置及 Gas 预约仍保留。

## 失败尝试与预算记录

首次真实钱包尝试 `usdc_cbd800894b0f0a8ba60438f6ef93c04ecfde30212e9559eae47878ff9e26eae2` 创建了发票，但外部 QA helper 错误拒绝 SDK 2.16.2 的 address-balance Gas（合法 `gasData.payment=[]`），在签名前停止。原发票过期且全节点仍 unpaid、点券未到账；没有伪造付款成功。[失败总结](live-failed-first/failure-summary.json)、[到期核验](live-failed-first/invoice-after-expiry-audit.json)保留原结果。

修复限于外部验收助手，独立 [25 项验证](live-failed-first/address-gas-helper-fix-validation.json)全部通过；不把它们加进产品 Node 22。之后[显式预算延续记录](live-real/reviewed-rollover-operation.json)保留原创建预约，把发票累计上限从 50m 调至 100m MIST；setup 140m + 两个发票各 50m = 240m，仍在本次 250m 测试预约上限内。没有重置旧预约、改游戏余额或修改旧订单期限。

旧失败发票实际 Gas 4,815,356 MIST，独立设置实际 Gas 30,550,599 MIST。买家的另一次 **0.01 USDC 普通 transfer**不是本合约充值，不能计入成功订单；成功单的比较起点因此为 19.99 USDC。

当前最新本机测试二进制：

- 最终 Gateway v4：`a634fb47ffa7be702953166e9860a6d375b4cd8c30d15be0b9d468f64422734f`，129,529,344 B。
- 实际付款 Gateway v3：`cdb9ab283ef032cf2b9c775ec7b74bbf5e4aa96b06881f9451eac7f7810158ca`，129,528,320 B。
- Windows：`4c8ab7bb76c842eaa3fd2166efe3316baa0887d3b085b362d30e65a3a25bab4b`，132,312,064 B。

环境门禁检查 `MIR2_RUNTIME_ENV`、`MIR2_DEPLOYMENT_ENV`、`MIR2_ENV` 全部变量；任一规范化值为 production/prod/staging/stage/preprod/preproduction 即拒绝，恢复模式也不绕过。

## 复核与文件完整性

[manifest.json](manifest.json) 记录本专题每个文件的原始字节 SHA256，以及最终实现源码按 UTF-8、CRLF→LF 规范化后的 SHA256。专题 `.gitattributes` 固定 `* -text`，避免 Git 的换行转换破坏图像及原始日志收据。manifest 不哈希自身。

```powershell
node docs/generated/sui-usdc-recharge-v1-20261029/generate-manifest.mjs --verify
```

SQL 脚本可在独立依赖目录按 [package-lock.json](sql/package-lock.json) 安装固定 PGlite 0.5.13 后执行；也可用 `MIR2_PGLITE_MODULE` 指向已有同版本模块。脚本只创建内存数据库，加载本仓库 SQL，不连接本地 Docker 或原生数据库。结果会更新 `sql/result.json`，若变化应重新生成 manifest。

维护总览与恢复手册：[Sui USDC 充值第一版](../../SUI-USDC-RECHARGE-V1.zh-CN.md)。机器可读计数、分类与待验收边界见 [evidence-index.json](evidence-index.json)。任何后续代码/文档/证据变化都应重跑 manifest 生成与校验，不能把旧源码检查静默转记为新代码全绿。
