# 统一测试版 r16：构建、发布与 Windows 验收证据

本轮发布及验收完成于 **2026-11-01 05:41 UTC 后（北京时间 13:41 后）**：远程 TEST 后端、完整安装包、六个公网/源站频道、Windows 更新、手机登录和测试 USDC 入账均有实际回执。临时 publisher 随后移除。本页不代表全项目 Accepted。

源码：`e9117b6691e3f7ec238ff1c1eb77b699b0646c2f`，维护分支 `codex/windows-player-journey`。本页整理时重新执行 `verify_unified_codeline.py` 通过，五个固定集成祖先齐全。参见 [统一代码线说明](../../UNIFIED-CODELINE-20261101.zh-CN.md)。构建快照 `C:/m2r16` 是该提交的冻结输入，不是另一条维护分支。

## 已实际完成

| 范围 | 本轮结果 | 外部证据编号 |
| --- | --- | --- |
| 远程测试后端 | Gateway、认证、worker 均已核验为 e911；迁移账本 28→33，旧迁移行、既有业务和序列保留。协调者本轮原库盘点为 127 个账号、41 个角色；最终回执确认原业务保留，未执行财务操作或策略变更。 | B、S |
| Windows 完整 Candidate | `WN-CANDIDATE-20261101-unified-16`，255,079 文件，strict full v4/CMS 验证通过；1,480 个编译输入与干净源码证明通过。 | W |
| 安装包与隔离安装 | 普通包及独立 QA 包均为 `2026.11.1.16`；QA 实际安装 exit 0。安装后 Launcher `--verify-only` exit 0，四个载荷摘要前后相同，未启动游戏；这一步是本地验证，不是公网更新验收。 | W、I、L |
| 工具栏实际操作 | 7 组鼠标按下/释放期间无移动发包或 movement hold，位置保持 `(288,616)`；截图支持 Small→Medium→Large→Small、行会过滤和设置窗口。随后普通地面点击向右移至 `(289,616)`，78ms 后收到服务端确认。 | T |
| 新后端药店实际购买 | 同一正常测试角色重连 e911 后，左商品列表与右背包可见；购买 40 金币药品，金币 60→20、快捷栏药品 1→2、背包药品仍为 11；旧“结果未确认”提示消失，恢复 `Select an item`。仅使用正常任务奖励金币，无经济覆盖或管理员操作。 | P |
| Packed r16 | 序列 `2026110104`，255,063 资源、34 packs；CMS、全部资源及干净的游戏/引擎 e911 来源均验证通过。K 的 `published:false` 是签名前后构建检查点，实际发布由 V 另证。 | K、V |
| 公网和源站 | 106 个原生 R2 对象闭包通过；65 个新对象完整公网字节与 41 个已批准旧不可变对象复用通过。18 组公网 CMS 在 Windows 验证；六个 R2 条件指针、源站六个原子成对选择器完成提升；12 个公网别名 GET 哈希、12 个公网 HEAD no-store/序列、12 个源站 GET 哈希通过。 | R、C、V |
| Windows 公网更新 | 正常已安装 Launcher 识别、确认并应用 `r2026110104`，验证资源并原子切换后从新 generation 启动至简体登录；选择器 healthy，重新 `--verify-only` exit 0。明确预热隔离内容缓存，实际元数据流量 10,796 字节、下载 packs 0，不是 Windows 冷下载。 | A |
| 分发路径修正 | 同一维护分支提交 `83a8f64aeee045dd8ccb96e2cd0dc7616e901ce2` 已推送；仅开放规范 packed 签名 feed 两个文件、各限 32KiB，不开放其他内部文件。原 404 先复现，修复后完整 40/40；实际 worker `b9677c0f-1a90-41d6-8158-478fa9e64258`。 | D、V |
| 手机扫码登录 | 已观察原生二维码创建、到期、刷新；随后用户在手机完成钱包签名，Windows 正常显示 `mir`（角色索引 39、战士 9 级）并进入远程世界比奇 `(330,268)`。签名由用户执行，助手未批准钱包签名。 | U |
| 测试 USDC 充值 | 原生 USDC 页显示 100 点券、待到账 0，唯一订单显示点券已到账；重复检查仍为 100。链上测试交易成功，数据库确认点券 0→100，订单/交易 claim 各唯一，来源额度标记为 `test`。未使用真实资金、未开启生产收费。 | U |

工具栏 T 的操作发生在旧 `13fc2e9f…` 后端上；P 才是客户端和后端均 e911 的药店验收。T 中命名为 `trade-held-drag` 的输入只记录到 20ms 按下/释放，且日志不含指针轨迹，因此不认定为持续长按/拖拽验收，也不认定双人交易完整闭环通过。

U 验证的是 Sui 测试网 **`direct_transfer` 充值**：1 测试 USDC 对应本次测试套餐 100 点券，`invoiceId` 为空，未走发票 registry。它与早期 Dubhe 个人求购订单、采购 Gas 测试及发票支付模式是不同路径；本次成功不能作为这些路径的新验收。法币充值本轮仅检查入口展示，未新付款；月卡未购买。二维码图像、会话内容和凭据不纳入仓库。

## 制品身份

| 制品 | 字节数 / SHA-256 |
| --- | --- |
| 普通完整安装包 | 1,026,799,208 / `4E56E9D6F30FF02FFB07ACBD78C5F29450387313747AD60D9D3937D452C7D1BE` |
| QA 完整安装包 | 1,026,799,613 / `550D34215EF874AAEE1E0642D131D6B5CE4B8D30BEB9C0E8888593F1CB183BCD` |
| Windows Game EXE | 133,747,712 / `1A008993D3C55C394FBD8E3A57A588D5056362B63452ABB164075A319B8952BA` |
| VERSION.json | `85606530619C3DF66CD338E7AE6FB43B59DBF90DEBCC2872A10687B147DC6084` |
| Updater engine | 序列 `2026110103` / `AC461F6C19CCDE9250F7DFBA7F9849827F120DE50FE863A52BE969E662CEA16E` |
| Packed generation manifest | `56DDD5417A09A2515FECE9A36BE2607282084EC89058C412D2EDD7A7DCB21BC8` |
| 编译输入 inventory | `BC9FBCAD3493FBE07781C30B9F502AE748631169FEE3984D62867E7FBFCC8A51` |

使用既有内部 TEST CMS 签名；两份安装包的 Authenticode 状态为 `NotSigned`。TEST CMS 验证与公开发行者签名是不同事实。

## 失败保留与未验收范围

- 原 Windows 公网读取进程 `70368` 超时失败，材料保留。远端非特权 PUBLIC HTTP 进程 `1540400` 完整读取普通安装包并验证 whole SHA 与 16 分片，随后对象 65 的规范签名 feed 被分发器拒绝 404。修正路径后，以固定原源码/原始日志/回执 checkpoint `e47f156228cdc250faa2e36357e5064203336a1e622a94c7776df6521033c915` 继续：继承已通过的实际完整字节证明并新 HEAD 确认 ETag，补读 65–105。旧失败未改写，未把分块摘要拼接冒充整包 SHA。新读取器 `d0276afc…` 与原读取器 `593382db…` 身份分开记录。
- 最终完整公网字节验证地点是远程普通 ubuntu 的匿名公网 HTTPS；Windows 对实际下载的 36 个 CMS 文件独立验签，并完成实际启动器更新。预热缓存不冒称 Windows 完整冷下载。
- 六个频道、源站和别名回读均已由 V 完成。V 是临时 publisher 移除前的不可变回执，其 `temporaryPublisherRemovalPending:true` 随后的完成情况由 X 单独记录；不改写原回执。内存 token 会话已正常关闭。
- 手机签名、原生扫码登录及测试 USDC 到账已由 U 完成；法币新付款、月卡购买和生产资金支付不在本轮已验收范围。
- 此页没有 3,000–5,000 CCU、多机自动扩容或双人交易容量的验收结论，也不替代用户最终前端验收。

## 外部证据来源与摘要

原始证据保留在发布协调目录 **`E:/mir2-release/unified-playtest-publish-20261101`**（下称 `CONTROL_ROOT`）。表内路径均相对此目录；迁移证据目录时可按相同相对路径及 SHA-256 核对。仓库仅保存本页索引，不复制安装包、巨大 JSON、二维码图像、原始环境或任何凭据。这里的摘要绑定已读取的小回执，并非重新下载或重建制品。

| 编号 | `CONTROL_ROOT` 下的相对路径 | 回执 SHA-256 |
| --- | --- | --- |
| B | `backend-r16/ROOT-FINAL-BACKEND-VERIFIED.json` | `5DD2A1341165D66D13740535BDA301302D59A205E9A66913E384607BD412D57D` |
| W | `windows-r16/LOCAL-ARTIFACTS.json` | `07B2D572FE8303A881A67BC69B9ABE688503FBEEA4FCDEABE4CF5E8E2CFFA739` |
| I | `windows-r16/qa-install/install-completed.json` | `11A70ECB51971A5A539628841669AEC7836560388A4EFB18AE53CA5F048C70C8` |
| L | `windows-r16/qa-notes/INSTALLED-VERIFY-ONLY.json` | `7301113E0E7F557395D9CD58C4F17F6EF55BBC9251584C34468E9A0DEBBA85F3` |
| T | `windows-r16/qa-notes/toolbar-observation/RESULT.json` | `69E744EFE362CC5983F2442DA5C2F84FCCAA912FED06D65EF6224ED4ECA6A85F` |
| P | `windows-r16/interactive-unified-backend/RESULT.json` | `59CAA12546D846C220E9BB1FA4DCF48F781C41D6660626E96BE628D208698156` |
| K | `packed-r16/VERIFY-PACKED-GENERATION.json` | `8F1A19BF872A8079D60411893D7C9054B55216376F9340F406BA5C5F477EA2D4` |
| R | `web/r16-prep/pub-v3/root-r16-complete-native-r2-closure-verification.json` | `696B02A95C9492C694FFC5C3F365DB79D4564496881EA585CE9CD928234DD2B5` |
| S | `COORDINATOR-R16-LIVE-0427.json`，04:28:47 UTC 历史检查点，支持 Caddy 和当时的发布状态；其手机/读取进度已由本页后续事实更新 | `9C8B3E4DE703702058E9C2002452445065D5C57DD852156BA58C12343A9E03E3` |
| U | `backend-r16/payment-acceptance/ROOT-REAL-PHONE-PAYMENT-VERIFIED.json`，05:18:18 UTC 手机登录、链上与数据库支付核对及原生界面截图索引 | `80CDC7291EFE89FB635F1B93A56B9A2D48B9616BBE1D2705DB730B6DDE212B25` |
| C | `web/r16-prep/pub-v3/public-r16-remote-cms-actual.json`，65 新对象完整字节、41 旧不可变对象批准复用及 18 组独立 Windows CMS | `C22BE8333D16152265648498D7A5B3FBF55DB84C8072533849AE46A52F478E74` |
| V | `web/r16-prep/pub-v3/ROOT-FINAL-PUBLIC-R16-VERIFIED.json`，六个实际 R2/源站选择器、双侧别名、文件及最终健康汇总 | `2C89A5802868479F8A684C7E9835C4C17413E094061AF91FD328AF580330C30F` |
| A | `windows-r16/interactive-public-updater/RESULT.json`，实际 Windows 更新、预热缓存边界及 healthy/再次校验 | `56D5194C51F1A4DDCDEEBFAE1E0D111F431303B487501135B9D505B334F2E3F6` |
| D | `web/r16-prep/reader-packed-feeds-tests40.json`，原 36 组及新增边界测试共 40/40，通过源码摘要固定 | `DBF72B3911E2F1858545F3194F619C2904ABA367B4CCB4AD0FB0D2E8561AF541` |
| X | `web/root-public-deployment-r16/session-01/temporary-publisher-removal.json`，核对原服务版本后删除临时 publisher，exit 0、未用 force | `60A30C764AEA91F4A0B7BA3998750E19623149D6DAFE71E2B9B66479540530B7` |

发布计划固定为 `web/r16-prep/pub-v3/release-r16-actual/plan.json`，SHA-256 `BEC26B4F531FC288A47D915673587FA69E4A3F016C7E8E8F376EB74D677AF596`。T 的固定 26 行原始日志前缀 SHA-256 为 `2D4161E6CFA773C0412EB4D109A3E9B3B84D9039D3EAB96607582FD93B2A8612`；原持续追加日志不以此摘要冒充全文件。P 的实际截图保留在同目录，未纳入仓库。
