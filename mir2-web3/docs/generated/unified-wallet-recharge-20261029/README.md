# Windows 扫码、简体、求购与充值统一证据（2026-10-30 完成）

**本轮有界集成/技术交付完成，`complete=true`；真人 Accepted 仍为 false。** 真实安装、16 张 Windows 原图、界面导航/简体重启，以及生产库本地 packed 应用和真实 Game 的 Ready→Healthy→正常退出均有证据。**既有 P2：语言菜单打开时点击穿透导致走一步，输入隔离未通过、未修复。** 本轮没有验证手机钱包签名、新付款、完整回滚、公开迁移或远程下载性能，没有公开发布或开启生产收费。Git 推送由提交后的外部回执记录，不作为自引用 artifact 的完成条件。协调说明见[本轮统一方案](../../UNIFIED-WALLET-RECHARGE-20261029.zh-CN.md)，完成口径见[有界完成记录](completion.json)。

保留双方历史的 merge 为 `e2c85519e9435ffbad34e449ba705e33e8c9ad13`；发现求购简体渲染缺口后，最终 Game 源码为其子提交 `1875c6c20e1285ffefc45ef6c21eab33e26fc536`。最终 Engine 源码与当前集成 HEAD 为 `77a2007a449d1f437a8d411bfa41f105be992fcf`。父提交及六份修复文件与 Git blob 的对应关系见[合并谱系](source/lineage.json)、[后端绑定](backend/committed-source-binding.json)、[简体修复绑定](procurement-sc/committed-source-binding.json)和[Game/Engine 分列](source/release-source-revisions.json)。本主题不把阶段报告内的旧快照哈希改写为新源码，也不把 Game1875 说成从 Engine77 重新构建。

| 合并后检查 | 结果与边界 |
| --- | --- |
| 后端 | [20 条测试命令](backend/validation.json)累计 365 个成功用例记录，按包、target 和完整测试名去重为 **359 个 Rust 身份**；不重复累加身份/攻击过滤器和 UID 子进程。simulation/gateway check 通过。涵盖 schema、交易 UID、Stripe/USDC、采购恢复、身份鉴权和共享世界相邻回归；原生 PostgreSQL 仅编译测试目标，未实跑数据库或链交易。 |
| Native 初始合并阶段 | [87 host + 2 相邻 + 117 client = 206 个逻辑测试通过](native/initial-validation.json)，默认 3 ignored。另执行 5 个 GPU 测试，共 64 张图；billing/procurement 的 GPU 单独执行，display GPU 本轮未重跑。原 7 张采购图是 **zh-TW**，不能算简体验收。 |
| 初始 GPU 图证 | [完整 64 张的文件清单/hash](native/capture-inventory.json)、[10 张代表图](native/included-screenshots.json)和 [AI 抽看记录](native/native-visual-review.json)。扫码二维码是保留域 `.invalid` 的离线 fixture；无手机批准或钱包登录。Guild 英文标签独立 GDI 对照只代表该测试范围。 |
| 求购简体修复 | [新阶段 13 采购 + 16 i18n 测试通过，GPU 1 项 / 7 张显式 zh-CN 图](procurement-sc/validation.json)。[五页与两次确认原图及核看](procurement-sc/curator-visual-review.json)系统文案使用简体；`龍城{Gold}`、原始装备描述、UID/操作码和运营套餐名保持原值。旧 9791 项与 873 条润色不变，新增 110 项采购键，总 9901；字体语料 2015 字符零缺字。此阶段与前 206/64 分开，不累加成全量重测。 |
| Web | [资源同步 464 records / 463 maps、Next typegen、TypeScript noEmit 三步通过](web/typecheck.json)。[依赖与锁文件保留](web/dependency-preservation.json)：Sui 2.16.2、wallet-standard 0.20.3、二维码依赖，以及原 Dubhe/采购固定声明保持；没有浏览器、Next build、服务或链上新验收。 |
| 最终 Game | [1875 干净构建证明](build/game-attestation.json)，`worktreeDirty=false`；实际 EXE 重新核对为 **133,843,456 bytes**，SHA256 `85879bfeb1b44d4c759e6a338338bb58a383b7701f886d1090a96b6267f00c6f`。没有将内部 Candidate 构建说成 Publisher Authenticode 已签名。 |
| 最终 Engine 与归档 | [Engine77 执行结果](updater/execution.json)、[9 文件 bundle 校验](updater/bundle-verification.json)与[精确签名指纹](updater/signed-artifact-fingerprints.json)。Engine 2,184,192 bytes / `c7b2a209…cedb`，Launcher 512,512 bytes / `e6442339…6d96`；新 archive 1,056,931,241 bytes / `119f9039…5d55`，255,082 文件 = Game255,079 + Engine3。Python 自身 `cmsVerified=false`，CMS 来自另行执行的原 bundle verifier 和 staged Game preflight。该阶段为内部 TEST CMS/`NotSigned` 本地构建，实际安装另列，无公开发布。 |
| 最终路径检查器 | [r5 synthetic 自测 21 项](checker/selftest-r5.json)通过，脚本 SHA256 `0cd66bd23670e3dd9fb73db32d996f3e8a09ac6e3dacc927716ed21c6c2aeead`；[固定实施计划](checker/implementation-plan.json)仍 `draftOnly=true/executed=false`。[早期失败与中间19项阶段](checker/development-history.json)保留。没有据 synthetic/坏签名拒绝样例宣称真实 Candidate 扫描或正例 CMS 已通过。 |
| r6 与实际最终根检查 | [r6 计划](checker/r6-plan.json)保持独立：仅删除“重复读者日志必须非空”的条件，保留真实 producer 返回、路径/时间/hash 等所有绑定；[25 项 synthetic](checker/selftest-r6.json) = 旧21 + 新4项时序样例，未累加到 r5。[真实最终根收据](candidate/final-targeted-verification.json)实际通过全 255,079 文件哈希，以及 Python 前后 native/provider ADS、reparse、精确文件/目录 allowlist、独立 CMS 和 clean source。 |
| 正式 Candidate producer | [原始验证报告](candidate/original-producer-verification.json)逐字节复制，SHA256 `7af3e30aa468d17a9ffcfd8a93d860d9b5c555e1a95630028fc656b2f57c5114`；`requireCurrentSource=false`、原 staging GUID、`currentSourceValidated=true/clean=true` 均未改写。[producer summary](candidate/producer-summary.json)和[小文件绑定复核](candidate/curator-binding-check.json)另存。255,075 条 manifest 项与实际总文件 255,079 分开记录。 |
| 实际 packed 签发/全验证 | [汇总](packed/summary.json)、[签发](packed/signing.json)、[独立验证](packed/verification.json)及[实际资源校验](packed/signed-resource-verification.json)：sequence **2026102912**，**33 packs / 255,063 resources**，1,069,032,581 pack bytes，`metadataOnly=false`。manifest `9dfbd083b6b92972539bc01816645ed6131297bb7ca4dedec659438987a74186`，catalog `1466ee20a770d5e2eef87d27eae5eb8b78ac4f8a539248679dc5a9629eef17f7`；不提交约69MB catalog、约37MB inventory 或真实包二进制。此阶段只代表签发/验证，本地应用另列。 |
| 两份完整 Inno 安装器 | [编译汇总](installer/build-summary.json)、[普通版](installer/normal-compile.json)、[隔离验收版](installer/validation-compile.json)均实际通过，分别 58.5 / 59.6 秒。23 份 recipe/语言/README 源码复制逐字节一致，输入 Game255,079 + updater9，两个 include 同 hash。普通版 **1,024,372,943 bytes / `387fd714…647f`**；隔离版 **1,024,372,706 bytes / `61526737…d1d3`**，版本 2026.10.29.11，`NotSigned`。这是本地完整编译，安装/GUI 由后续独立阶段记录。 |
| 真实隔离安装与启动 | [实际安装](windows-ui/actual-installation.json)exit0、259.37 秒；[安装后完整内容验证](windows-ui/full-installed-verification.json)255,079 Game 文件哈希及 9 updater 文件通过。[实际父子进程链](windows-ui/observed-launch-chain.json)证明无参数 Launcher→Engine→Game，无直接 Game 启动或 assetRoot override。三份 launch-request 仅是请求记录，实际 GUI 另由图片和下行汇总证明。 |
| 实机 Windows 导航 | [实机汇总](windows-ui/actual-ui-summary.json)和[16 张未改 JPG +16 原始 sidecar](windows-ui/screenshots-index.json)覆盖钱包入口、三充值页、普通 NPC 求购五页、十语菜单、简→英→简与保存简体后实际重启；本 curator 逐张核看。显示/导航范围通过；**语言菜单输入隔离 false**，NPC `Personal procurement` 与部分服务器字符串仍英文，未泛称全中文或全部输入行为通过。 |
| 真实本地 packed 应用 | [应用汇总](packed/local-application.json)、[LOCAL-APPLY](packed/actual-local-apply.json)、[真实 READY](packed/actual-real-game-ready.json)、[HEALTHY](packed/actual-real-game-healthy.json)和[正常退出](packed/actual-real-game-exit.json)：真实 `generation::check_update`/CMS/入场控制，经 Pending→真实 PID/nonce 认证，Ready 5.229 秒、再存活至少10秒→Healthy，exit0，未杀健康子进程。**57.358 秒是 LocalFiles 本地应用耗时，不能当远程/CDN速度**；Ready 未模拟。 |
| 原始日志固定 | [67 份已有日志的长度/hash 复核](source/external-log-receipt-verification.json)通过。此处只重算已有文件，不重跑测试；原始日志留在外部 BASE，避免将账号/认证日志带入仓库。 |

初始 e2c Game 曾完成干净构建，但[源路径核查](history/procurement-sc-gap.json)确认求购直接创建 `Text`、未走 keyed 本地化，所以在 SC 下仍显示繁体。协调者在签发 Candidate 前[停止并替代该打包阶段](history/package-v2-superseded.json)；[旧构建证明](history/game-e2c-superseded.json)保留为历史，不能当最终交付。修复阶段的首次编译错误和后续真实通过均保留在其原始收据中。

实际 UI 使用[独立 synthetic fixture](windows-ui/synthetic-fixture.json)，仅复制测试角色的 **100 点券 / 10000 金币**，该 fixture **0 张历史 USDC 订单**；[loopback Gateway](windows-ui/server-process.json)源码为后端未变的 e2c，支付 provider 与链上采购 driver 关闭。[经济状态对账](windows-ui/economy-conservation.json)确认前后金币、点券、背包、billing、procurement 和月卡状态 hash 均未变，0 次付款、0 笔链交易；登录绑定和角色位置变化不纳入经济守恒。原含 3 张真实历史订单的库保留未改，其恢复驱动关闭时被 `usdcRecoveryDriverRequired` 正确拒绝，见[边界记录](windows-ui/recovery-driver-boundary.json)。界面截图只证明这套 fixture 的显示/导航，不能替代新 USDC 付款、采购成交、真实手机扫码登录或原库恢复验收。

[P2 核查](windows-ui/known-language-input-p2.json)明确两次打开语言下拉各让角色走一步：`27,42→27,43→27,44`；选语言本身两次未观察到额外移动。这套机制在 b87、d7 双亲已存在，不是本次 merge 新回归；**问题仍未修复，`inputIsolationPassed=false`，不能据界面显示通过宣称前端 Accepted**。

packed 本地 fixture 只从真实安装后的受验文件复制[最小受认证 baseline](packed/minimal-baseline-preparation.json)：Game EXE/控制 metadata 和 9 个 updater 文件，**没有复制松散资源，也不是注册安装的完整副本**，因此真实 Game 必须挂载 packed 资源。[外部 driver 准备](packed/driver-preparation.json)的旧 pending/apply=false 状态保留；实际执行和[control/source 绑定](packed/local-fixture-control-bindings.json)分列。生产库 admission、真实 ReadyServer 的 PID/nonce 与 Healthy 都实际执行；完整 legacy/crash 回滚、旧 cached Launcher 无参数自动迁移和公开用户迁移均未做。原始 READY/HEALTHY attempt 值只存 SHA256，原始 Game stdout/stderr、control 环境、账号与密钥均不入库。

[原始工作树](source/original-worktrees-preservation.json)的 tracked 补丁 hash 与初始状态一致，私人补丁内容未发布；[实机后保留检查](windows-ui/post-ui-preservation.json)只证明既有 D 盘普通 Game EXE 与三份个人偏好 hash 未变，不是 D 盘全目录审计。测试 Game 窗口已关，[仅本轮 Gateway/loopback 测试进程清理](windows-ui/process-cleanup.json)完成，Docker 与公共服务未动。

[重复 reader 停止记录](candidate/duplicate-stop-observation.json)明确 `outerWrapperPassed=false`：真实 producer 完成后，协调者只停了自己启动的冗余只读 verifier wrapper；不能把被停的外层脚本写成 exit0。原 producer 真实返回与最终根检查是另两份证据。检查器与 Python 只读核验不依赖削弱现有 package 门禁。[独立 F:/ug source-only 摘要](source/game-sparse-source-summary.json)核对 1343 份编译输入、源码洁净；这不是另一次游戏重编译。

| 检查 | 已有结果与边界 |
| --- | --- |
| 发布模块源码回归 | [阶段汇总](release/validation.json)，[50项模块快照](release-implementation-summary.json)描述当时未提交合并阶段，未冒称最终干净源码构建 |
| Updater Rust | [lib126通过/8 ignored、启动健康3通过](release/rust-tests.json)；另四项签名集成测试默认 ignored，未计通过 |
| 已有 r10 的真实 CMS | 同一 Rust 记录中两项只读检查单独执行通过：旧完整 ROOT17 manifest、签名 seed/floor、EXE/attestation；未启动游戏，未写 receipt/floor/selector，也不代表新 generation 已验 |
| Python发布链 | [8组186项中183通过、3 skipped](release/python-suites.json)；Archive/Bootstrap/传输和发布 fixture 的 Mock/占位签名不能当新真实 CMS 或生产发布 |
| 精确文档布局 | [39项](release/legal-layout.json)与[两份生产allowlist函数74项](release/package-allowlists.json)通过；该源码阶段没有完整 package/实际包验收，真实255,079文件检查由上方独立阶段记录 |
| 旧 ROOT12 兼容 | Python三项使用固定 Wallet28 原始 archive parser，接受五份新 legal 路径并保留危险路径拒绝；尚非旧原生二进制的实际迁移 |
| Inno十语静态闭包 | [10×281条可信编译器消息及每语15条Custom](release/inno-static.json)通过；没有把静态验证当实际安装器编译 |
| 开发失败保留 | [零测试、旧字体复制失败、回归测试缺import等阶段](release/development-history.json)保留原日志长度/hash与原因；最终通过不抹去早期失败，也不累加中间计数 |

更新器保留 engine2 的资源包读取、健康握手与回滚，也保留 r10 ROOT17、十语严格偏好和简体进度。只有有效 CMS、有效 schema/期限、严格低于现有 signed legacy floor 的 feed，且没有 packed selector/receipt、没有待恢复 legacy 更新、重读 floor 相同并验证旧 Game，才允许继续启动该受验旧安装；跳过 v4 二次网络发现。同序列、坏签名、畸形输入、坏包与未知字符串错误不使用此路径。新包五份合法文档移至精确 `mir2-assets/legal/`，同一字节和 OFL 仍须全 hash；完整旧 root 布局继续接受，partial/mixed/duplicate 拒绝。

Candidate 标识 `WN-CANDIDATE-20261029-unified-11`、安装器版本 `2026.10.29.11`；Engine bundle 已实际使用 sequence `2026102911`，更高 packed `2026102912` 已独立本地签发、全验证和应用，未发布。实际签名 metadata 的 VERSION SHA256 为 `51afe182412b0cd7dabeb23228f3cce354d82db9694062fa40fab1166397308a`。[原始 pipeline 收据](updater/pipeline-result.json)与 [staged preflight](updater/staged-game-preflight.json)中的历史 `producerPackagePending=true`、`installOrPublishAllowed=false` 原样保留；后续[安装器入场收据](candidate/installer-admission.json)只代表当时准许编译，真实安装/GUI 用各自后续证据证明。packed 的[未签准备收据](packed/prepared.json)仍保持当时 `unsigned=true/cmsVerified=false`，后续签发、验证、实际本地应用分列，未伪改历史状态。后续签名前仍须核对已签 seed/receipt 的最高 floor；不能删除防回滚状态、复用不同字节的同序列，或用高序列旧 Game bridge 覆盖 r10。公网 old/Unified feed 均未改；r10 和旧扫码版本不能靠一个严格 baseline 维护 feed 自动同时升级，公开部署另行处理。

[index](evidence-index.json)列出必需的新合并后门禁。目前只收小报告和必要截图，原日志以长度/hash固定在外部 BASE；不提交账号、私钥、私人配置、原始认证日志、EXE、安装包、pack 或临时工具。旧 [r10 finishing](../native-zh-cn-finish-20261029/README.md)、[简体](../native-zh-cn-20261029/README.md)、[充值导航](../billing-entry-ui-20261029/README.md)与[USDC真实测试](../sui-usdc-recharge-v1-20261029/README.md)均保持历史快照，不回写其 manifest，不将旧成功加到本轮统一实测。

已在[当前集成源码账本](source/current-integration-source.json)固定 HEAD77 的 447 份受管源码输入，并与真实 Git blob 比对；只纳入受 Git 管理的输入，本次没有排除项。发布模块 50 项、Native 366 项 snapshot pins 只描述其测试阶段输入，不能改称后续源码全量重跑。最终 `manifest.json` 固定本轮有界技术证据与源码，`--verify --require-complete` 已通过；协调者 stage 后再执行 `--verify-staged --require-complete` 检查真实 index blob。本轮未替换既有普通安装，未做 r11 卸载重装；旧 r10 的卸载重装不能计为 r11 验证。人工语言/前端 Accepted、手机签名、原生 PostgreSQL、完整回滚和公开发布仍分离，不因本主题 complete=true 而视为已通过。
