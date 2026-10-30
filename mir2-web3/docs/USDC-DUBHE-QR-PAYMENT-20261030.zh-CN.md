# Dubhe Wallet 手机扫码充值候选（2026-10-30）

状态：**本地候选实现和受控验证完成，尚未发布或完成手机实签。** 原测试服仍为 `050950f5b…`，本轮没有改变服务、账号、角色、订单或财务数据，没有启动 Docker，没有发布客户端更新。

## 实现范围

原 USDC checkout 增加 Dubhe Wallet 手机扫码为默认方式，浏览器扩展仍可选择。采用钱包已有的 WalletConnect v2 通道；连接只申请 `sui:testnet` 的 `sui_signPersonalMessage` 与 `sui_signAndExecuteTransaction`，不能将 Dubhe Connect 登录二维码当成付款授权。

扫码后，手机先签原订单的付款钱包确认消息，原 Gateway 验签后固定 payer。新 `buildPayBytes` 复用原 USDC/invoice/merchant/amount 构造，在已核实 Testnet 的 SDK client 上构建完整 BCS；核实付款人自付 Gas、预算、实际 Gas 所有权/余额、64 KiB 上限和构建前后未付/未过期。它不加载付款私钥，不签名、不广播。手机确认并自行执行后，返回字节须与请求完全一致；digest 仅作原 `submitted` 提示，链上 checkpoint、invoice、事件、付款人和商户净余额变化仍由后台独立核账后幂等发点券。

二维码只含 WalletConnect pairing URI；单订单 capability 保留在 checkout 内存，仅经同源 Authorization header 使用，游戏 session 不进入钱包或 relay。SDK 和 QR 库按 lockfile 构建成同源静态 bundle，不从远程 CDN 加载脚本。CSP 只额外允许固定的 WalletConnect relay 与本地 data 图片；遥测关闭。取消、过期、换钱包、迟到返回和页面关闭会撤销本页连接，已付款/已提交/未知订单均不允许再次发起付款。

## 缺少的外部配置

充值页 Gateway 配置名为 `MIR2_SUI_USDC_WALLETCONNECT_PROJECT_ID`，是 32 位 hex 的公开应用标识，不是私钥或支付授权；缺失/格式错误时不生成假二维码，不发起手机付款。原登录、报价和扩展入口不依赖该值。

本机及原测试服未查到可用项目 ID，Vercel 连接器本轮连接失败，不能据此断言 Vercel 中不存在。公开钱包页面静态核对也未取得可核实的就绪配置，仍需确认钱包端和 DApp 两侧能正常连接。不能填用别人的示例项目 ID。若钱包构建仍通过动态 `process.env[名称]` 读取浏览器配置，应在钱包仓库改为静态 `process.env.NEXT_PUBLIC_WALLETCONNECT_PROJECT_ID` 访问并重新构建，再做真实配对验收；本轮未修改或部署钱包仓库。

WalletConnect 是现有交易扫码的一种实现选择，并非 Sui/USDC 的硬性要求。若采用 Dubhe 自有交易二维码，需要另行扩展钱包扫描识别、交易审批、完整交易/网络/钱包绑定、回传验签及终态幂等，不能复用当前只登录的 `/sessions` 或把普通批准回执当签名。

协议核对采用 [Dubhe Wallet 固定源码 f339af3](https://github.com/0xobelisk/dubhe-wallet/blob/f339af3251499f60ae1b56bf64742145afc2d6ce/packages/web/src/lib/walletconnectRuntime.ts)。该钱包的 WalletConnect 交易接口只接受完整 BCS base64，不能将原 `transactionJson` 直接传进去。

## 已执行验证

- Gateway Rust `sui_usdc` focused：19/19，通过新 operation、capability、配置拒绝、旧订单恢复和幂等核账；使用原生 Windows 工具链，不启动数据库容器。
- Node driver：29/29，含原有 23 项；新增测试使用真实 SDK resolver 与 BCS codec，覆盖 Gas 所有权/资金不足、错链、已付/过期及构建期间状态变化，不真实付款。
- 手机适配器 16 项、真实 checkout controller VM 14 项：共 30/30，覆盖官方参数、字节一致、错误账户/链、取消/过期/迟到、并发、已提交状态与页面关闭清理。
- Headless Chrome：桌面 1120px、手机 390px、缺配置三种检查通过；实际同源 bundle 加载与 QR 图片渲染、无横向溢出、无 pageerror；配对/订单为 mock，无外部 relay 连接。截图明显标注“模拟订单和二维码，不能付款”。[证据](evidence/usdc-dubhe-qr-20261030/BROWSER-MOCK.json)
- npm production dependency audit：0 个报告漏洞（当次检查），不是永久安全保证。

bundle 来源和哈希见 [构建收据](../apps/gateway/src/sui_usdc_dubhe_wallet.bundle.json)。发布流水线增加 npm lock 安装、重建比对、上述 Node/controller 回归与 Gateway focused 测试，再进入既有 Linux 构建/原生 PG 发布门槛；本轮尚未 dispatch Linux 发布构建，也未重启原服。

## 待验收

取得有效项目 ID 并确认钱包端配置后，先完成精确源码 Linux 构建与隔离候选检查。再在当前原服配置快照和完整备份基础上，按空闲/正常保存门槛切换精确 Gateway 及匹配 driver，仅精确新增同源 `dubhe-wallet.js` 路径，不动 wallet-login、旧推广后端、返利付款或另一个会话的客户端分发。上线后由用户完成手机配对、拒绝/断开，以及审阅并确认一笔 Testnet USDC 付款，分别核对链上交易与游戏点券；这些尚未验收。
