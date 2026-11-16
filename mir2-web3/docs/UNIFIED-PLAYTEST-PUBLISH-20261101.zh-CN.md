# 统一版本测试发布（2026-11-01）

状态：**统一 TEST r16 已发布，远程后端、六个公网更新入口、源站和完整安装包已核验；Windows 实际更新、手机扫码登录和测试 USDC 到账通过。**

游戏、更新引擎及后端冻结源码为 `e9117b6691e3f7ec238ff1c1eb77b699b0646c2f`。
分发器另有必要修正 `83a8f64aeee045dd8ccb96e2cd0dc7616e901ce2`，已在同一维护分支提交推送。
逐项范围、回执摘要和未通过边界见[本轮证据](evidence/unified-playtest-publish-20261101/README.md)。

用户授权更新现有远程 TEST Gateway、独立扫码认证服务、配套 worker、公网 Windows
更新频道和完整安装包，并在发布后由助手操作 Windows 验收扫码登录、充值、药店窗口及
聊天底部按钮。保留游戏数据与既有计费策略；不启动本地 Docker，不部署主网或使用真实资金。

## 发布前修正

- 独立 wallet-login 与 Web 共用认证实现。两条 POST 路由显式只传入 Request，避免把
  Next 的第二个路由上下文当成认证配置。新增三个真实路由/认证回归和完整类型检查通过。
- 完整安装器支持显式候选身份、发布版本/文件名和隔离 QA AppId。默认旧配方兼容，
  正式 AppId、签名、全文件闭包及来源校验保留；不在 staging 副本悄悄替换源码。
- 新安装基线不能使用只认识三个历史 VERSION 哈希的维护发现逻辑。Updater 由构建参数
  精确绑定当前 Candidate VERSION.json SHA256，额外历史基线必须由原签名元数据验证。
  编译期列表最多 64 个，非法/重复列表整体拒绝；运行时环境不能替换它。三个历史频道保持
  原映射，新基线使用独立的 `/client-updates/unified/baselines/<SHA256>/` 命名空间。
  后续引擎须保留仍在支持的已安装基线，按实际基线选择其自己的频道，避免新引擎切断旧客户端。
- Native R2 只读分发器支持上述精确大写 64-hex 基线作用域；可变 latest 仍走独立原子指针
  并禁止缓存，既有 root/five-class/unified/packed 命名空间及检查保留。
- Linux backend-build 的原生 PG 窗口补充 wallet、promotion 来源、Zone 保存、邮件、UID 和
  击杀事实组；拒绝 SKIPPED、零执行、失败或 ignored，逐组绑定日志哈希。

本轮原始日志集中在本机 `mir2-release/unified-playtest-publish-20261101`。发布二进制、
安装包及各入口的最终 SHA、渠道序列和验收结果完成后写入本主题证据，不能用旧截图、
旧 PG 测试、旧 ELF 或旧 Windows EXE 冒充本轮发布。

## 已执行与核验

| 范围 | 本轮结果 |
| --- | --- |
| 后端 | 实际 Gateway ELF、独立扫码认证服务和 worker 均为 e911；原库备份已恢复演练，28→33 完整迁移行与既有业务保留，最终运行回执通过。发布后 Gateway/认证健康仍为 e911。 |
| 安装器 | `2026.11.1.16`，普通包 1,026,799,208 字节，SHA-256 `4e56e9d6f30ff02ffb07acbd78c5f29450387313747ad60d9d3937d452c7d1be`；完整 Candidate、QA 实际安装和安装后校验通过。 |
| 公网文件 | 106 个 R2 对象闭包通过；65 个新对象完整公网字节验证，41 个旧不可变对象按已批准的原生校验和、旧签名证明及新公网边界读取复用。18 组实际公网下载 CMS 在 Windows 独立验证通过。 |
| 六个频道 | packed、当前安装基线、保留 r15 基线、unified、fiveclass、root 均按原 ETag/不存在条件提升；源站六个原子成对选择器同步提升。12 个公网别名完整 GET 哈希、12 个公网 HEAD 的 no-store/序列、12 个源站完整 GET 哈希通过。 |
| Windows 更新 | 已安装 Launcher 实际发现并应用 `r2026110104`，资源校验及原子选择器成功，从新 generation 启动至简体登录界面，状态 healthy，随后 `--verify-only` exit 0。 |
| 手机扫码 | 用户在手机完成真实签名，助手观察既有角色 `mir`（战士 9 级）并正常进入比奇 `(330,268)`；未注入账号凭据或代签。 |
| USDC 到账 | 本轮新单 `usdc_9f56be8653b9…` 支付 1 测试 USDC，原生余额 0→100、待入账 0，重复检查仍 100。链上 SUCCESS、收款方与币种/金额、原生 PG 单一 claim/来源 lot 和存档 100 一致。 |
| 药店与底栏 | 同一普通测试角色在新后端可见商品和背包、正常购买药品并关闭窗口；底栏点击及地面点击正对照按证据记录，不扩大为双人交易或全屏像素验收。 |

[下载 r16 完整安装包](https://assets.mir2.obelisk.build/client-updates/installers/4e56e9d6f30ff02ffb07acbd78c5f29450387313747ad60d9d3937d452c7d1be/Mir2Setup.exe)。
现有支持的客户端从正常启动器查看更新；维护序列 `2026110103`，资源包序列 `2026110104`。
安装器使用既有内部 TEST CMS，外层 Authenticode 仍为 NotSigned。

## 验收边界与恢复

- Windows 最初完整公网读取超时；保留失败材料。之后由远程普通 ubuntu 用户读取相同
  匿名公网 HTTPS，完成安装包及新文件完整哈希。首次远程读取在签名清单 404 处停止，
  修正受限路径后，从固定原始哈希、16 分片断言日志和对象回执继续，未重读或伪造已通过的 1GB 证明。
- 分发器仅新增规范 `packed/feeds/s<安全正整数>-<小写 SHA>/latest.json|p7s`，各限 32KiB；
  不开放目录、内部指针或其他文件。回归先复现 404，再完整 40/40 通过；实际部署版本
  `b9677c0f-1a90-41d6-8158-478fa9e64258`。
- Windows 更新验收使用明确预热并逐文件校验的隔离内容缓存，实际公网元数据流量 10,796 字节、
  新下载 packs 为 0；不能冒称 Windows 完整冷下载。完整公网字节与 Windows CMS/更新分别报告。
- 本轮充值是 Sui Testnet 原生 USDC `direct_transfer`，不是采购 Dubhe 合约或发票事件。
  交易 [Cysbi1…dS6oi](https://suiscan.xyz/testnet/tx/Cysbi1DNyG9T4jpBKQBkJFAg4EUBcH2EZrKMD1ZdS6oi)，
  净 Gas 2,322,532 MIST；使用官方 GraphQL 只读复核，旧 JSON-RPC 已返回停用错误。
  查询方式依据 [Sui 官方文档](https://docs.sui.io/develop/accessing-data/graphql/query-with-graphql)。
- 法币只验收入口与报价展示，未新付款；月卡独立展示且未购买。未开启主网、真实收费或付款。
- 旧版本、原选择器、失败 intent、原库备份及恢复材料保留。回退须按原操作回执核对当前 ETag/选择器，
  不重放 apply/promote、不用旧二进制直接配已迁移数据库。临时 publisher 已按原版本核对后移除，内存 token 会话已关闭。
- 全项目 Accepted、双人交易、全部 UI 像素一致及 3000–5000 战斗 CCU 仍没有本轮验收结论。
