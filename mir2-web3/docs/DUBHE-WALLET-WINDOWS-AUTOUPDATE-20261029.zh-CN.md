# Windows Dubhe Wallet 自动更新发布（2026-10-29）

本记录覆盖 Windows 旧客户端自动更新到 Dubhe Wallet 扫码登录版本。它接续
[2026-10-28 测试服更新](DUBHE-WALLET-PLAYTEST-ROLLOUT-20261028.zh-CN.md)；
服务端更新与 Windows 签名下载渠道是两次独立发布。

**Windows 签名渠道发布、真实旧客户端自动更新安装验证及临时发布资源清理已完成。**
origin 和 Cloudflare CDN 均使用最终 sequence 23 / 24；原启动器完成真实 23 桥接和旧游戏健康确认，
随后通过 C824 引擎实际安装并激活 Candidate28，全部 255,069 个载荷文件的大小与 SHA-256 校验通过，
个人文件和日志保留。临时发布入口与三份传输凭据已清理，清理后两个域名的 24 项公开复核再次通过。

Candidate28 的首次正常游戏健康确认、可见前端和 Dubhe Wallet 手机真人扫码签名仍待人工验收。
第二次安装使用 `--check-only`，没有启动 Candidate28，也没有接受其首次游戏健康状态。

## 版本与包身份

| 组件 | 固定版本 / 用途 |
| --- | --- |
| 游戏源码 | `33f0b8c252f65cc4a24b133292cd1f9eba122dae`；干净源码，Rust/Cargo `1.95.0` Release 编译 |
| 游戏候选包 | `WN-CANDIDATE-20261029-wallet-28` |
| 最终更新引擎源码 | `0164ae44637cbac06f967a5732ca62a74221063f`；支持新渠道、较大清单，并修正资源下载规划 |
| 测试服源码 | `00ca0f93f0c7e367afec495711e0e04334f4deb8`；沿用已部署服务，本轮未重新部署后端 |
| 发布辅助脚本源码 | `df29ba3559c3a10b49a4cb6f555fd1d9b2fcb391`；兼容逐字节固定的 R25 原 CDN 分发文件集，同时拒绝缺失、改动或额外文件 |
| 桥接游戏 | 原 `WN-CANDIDATE-20261011-invited-25`，游戏源码仍为 `d917de1ff4988e2d9fc7ca58f958cb5c80861fdf` |

组件源码分别固定。最终引擎升级没有重新编译 Candidate28 游戏；游戏 EXE、清单和签名继续沿用
`33f0b8c` 的已验收字节，不能将游戏来源改记为后续引擎或发布脚本提交。
最终引擎的真实构建、签名及干净源码准入见
[引擎升级准入记录](generated/player-qa/dubhe-wallet-windows-autoupdate-20261029/engine-refresh-admission.json)；
该记录证明构建准入，实际公网更新结果见后文两次运行的独立证据。

| Candidate28 文件 | 大小 / SHA-256 |
| --- | --- |
| `mir2-platform-windows.exe` | 115,421,696 bytes；`CE8A604A5A2F0A1185FA279E0299232DAB0E09F6498FBA9BE959F9D6A1F75440` |
| `PACKAGE-MANIFEST.json` | 49,712,342 bytes；`BCA71514A99E158EB583F48D3E81095219C81C0C122CB1AE498B081460F822C0` |
| `BUILD-ATTESTATION.json` | `63DCE46D47459CF79AF412BDDD9624CD36DC5F7C4F9201BA0FB1D276E4BDA241` |
| `VERSION.json` | `C54DC2C9D8301E1793B8316AFE6CAB554DF19FC62468D6005F41C7897DBC802F` |
| 最终更新引擎 EXE | 1,959,936 bytes；`C8241025681123CE3E9B24F3E78AAC17577B24833395F7BB2644B8905AAFC5DE` |

包内有 255,069 个载荷文件，最终候选包共 255,073 个文件。完整严格验收的
`passed`、`currentSourceValidated`、CMS 签名及 PE 检查均为真，源码无未提交改动。
原验收报告保留打包暂存目录；随后通过原子改名后的七个关键文件字节绑定，将该报告关联到最终目录，
没有改写原报告中的路径。

证据：[完整严格验收报告](generated/player-qa/dubhe-wallet-windows-autoupdate-20261029/candidate28-verification.json)、
[最终目录绑定](generated/player-qa/dubhe-wallet-windows-autoupdate-20261029/candidate28-final-binding.json)。
两份证据的 SHA-256 分别为
`4CDB98689B7FDEE725297670E832D3271E119A60121966A2188935485A7ECA66`、
`084D076B10F26784923A9E3FCAFB34131F4BAD38DB81DE7FF8C349E89EE1607C`。

## 渠道与旧客户端操作

新包清单约 49.7 MB，超过旧更新引擎的 32 MiB 读取上限，因此分两步发布：

| 渠道 | sequence | 游戏 / 更新引擎 |
| --- | --- | --- |
| 原 `client-updates/latest.json` | 23 | 原 R25 游戏 + C824 更新引擎，供旧启动器完成桥接 |
| `client-updates/five-class/latest.json` | 24 | Candidate28 游戏 + 同一 C824 更新引擎 |

旧 R25 的游戏及原签名交付文件保持原字节。新引擎支持 64 MiB 清单，下一次启动时读取
sequence 24；不会将 Candidate28 的大清单直接交给旧引擎。

sequence 21 / 22 是中间发布：使用来源 `1f2b71e`、SHA 以 `F2F1AB36` 开头的旧引擎。
真实更新检查发现 fallback 分发缺口和下载规划造成的逐文件请求过多，最终改用上述 C824 引擎。
origin 的 21 / 22 已被 23 / 24 替换；旧 CDN 根渠道跳过 21，从原 20 条件提升到最终桥接 23。
CDN 的 five-class 22 也已被最终目标 24 替换。

已带 `Mir2Launcher.exe` 的旧客户端按以下步骤更新：

1. 正常打开原来的 `Mir2Launcher.exe`。第一次更新桥接引擎，随后进入旧 R25 游戏。
2. 等待旧游戏完成启动并健康运行约 10 秒，再正常关闭游戏，让桥接版本完成健康确认。
3. 再次打开同一个 `Mir2Launcher.exe`，由新引擎下载并激活 Candidate28。
4. 到登录页后按 F8 打开 Dubhe Wallet 扫码入口；先在登录页选择语言，钱包登录界面沿用该语言。
5. 使用手机 Dubhe Wallet 扫码并确认登录签名，然后选择或创建角色。

应从启动器进入；直接打开旧游戏 EXE 不会执行以上更新。未携带启动器的更早安装包需要先取得带启动器的安装包。
不要在第一次桥接启动后立即强制结束进程，否则健康确认尚未完成。

## 测试服连接与职业策略

游戏连接 `wss://165.154.65.136.sslip.io/playtest/ws`，独立登录服务为
`https://auth-playtest.165.154.65.136.sslip.io`。
服务端继续使用 `platinum_176`；客户端包携带 `platinum_fiveclass` 的完整资源超集，
配置 `allowed_classes = ["Warrior", "Wizard", "Taoist"]`，只允许新建战士、法师、道士。
原画面中的刺客、弓箭手灰色槽位保留且不响应建角操作。

限制只作用于新建角色。已有刺客、弓箭手的角色列表与 `StartGame` 行为仍被保留，并有回归测试覆盖；
这些测试没有连接服务器或新建真实账号。没有裁剪五职业资源，也没有将测试服切换为五职业规则。

[职业策略和相邻回归证据](generated/player-qa/dubhe-wallet-windows-autoupdate-20261029/policy-verification.json)
记录共享策略 7 项、共享 native shell 86 项、Windows 策略 4 项、session config 15 项、
shell bridge 20 项、wallet login 21 项及真实离屏渲染 1 项通过；忽略项在原日志中保留。
[建角画面](generated/player-qa/dubhe-wallet-windows-autoupdate-20261029/classic-ui-01/creation-policy-three-class.png)
仅证明三职业按钮与原灰色槽位的渲染，不代表真人前端验收。

## 更新引擎验证边界

最终引擎的下载规划专项测试为 25 项通过、4 项忽略，断点续传回归为 14 项通过；
完整库测试为 87 项通过、5 项忽略。上述测试范围有交集，不将它们相加作为独立测试总数。
见[规划与断点续传验证记录](generated/player-qa/dubhe-wallet-windows-autoupdate-20261029/delivery-planner-verification.json)及
[最终引擎完整库测试日志](generated/player-qa/dubhe-wallet-windows-autoupdate-20261029/engine-s23-s24-full-lib-tests.log)。

使用真实已签名的 R25 与 Candidate28 数据另做了 planner 预测，结果单列如下。
该预测没有预填缓存、没有执行网络下载，也没有实际应用 delta。

| 验证类型 | 观察结果 | 能证明的范围 |
| --- | --- | --- |
| 真实签名数据的 planner 预测 | 选择 11 个资源包；剩余 153 条路径、151 次原文件请求；计入匹配 delta 后预计传输 654,357,224 bytes | [预测记录](generated/player-qa/dubhe-wallet-windows-autoupdate-20261029/delivery-planner-real-forecast.json)只证明规划输出；旧规划只选 2 包、剩余 33,992 条路径。新结果不是实际请求计数、实测传输量或耗时 |
| 真实旧启动器及公网更新安装 | 23 桥接、24 安装激活与全载荷校验通过 | [真实检查结果](generated/player-qa/dubhe-wallet-windows-autoupdate-20261029/real-legacy-launcher-public-s24.json)：5 个资源包、0 次 fallback、870 个下载文件，本次运行 wire bytes 为 397,833,195；复用了此前真实公网下载且校验过的缓存，不能作为冷缓存速度结果 |

上述实际安装没有手工注入 Candidate28。它复用了此前真实公网 HTTP 下载、随后由更新器验证过的本机缓存，
因此实际选包及本次流量与无缓存的 planner 预测范围不同，不能据此推算首次全新安装的下载量或耗时。
[23 → 24 最终准入记录](generated/player-qa/dubhe-wallet-windows-autoupdate-20261029/real-public23-24-e2e-admission.json)
将第一次真实桥接、第二次检查及[原始运行日志](generated/player-qa/dubhe-wallet-windows-autoupdate-20261029/real-public23-24-last-run.jsonl)绑定到固定 SHA。
第二次结果为 `activated=true`、`transactionPhase=committed`：实际改动 101,918 个游戏文件，
完整验证 255,069 个载荷文件、1,202,414,057 bytes，保留个人文件和日志。
`gameLaunched=false`、`firstLaunchHealthAccepted=false`，首次游戏健康确认仍然保留为待完成状态。

Candidate28 的 origin 已补齐与已签名计划一致的 21 个交付对象：18 个完整资源包、1 个 delta、
`DELIVERY.json` 与其签名。补齐前逐项校验 size/SHA-256，先私有验证、再 create-only 发布，
未覆盖既有文件；随后 21 项公开 HEAD、两份元数据的完整 GET/SHA 和 19 个资源包的 Range 206 / 前缀检查通过。
见[origin 交付补齐回执](generated/player-qa/dubhe-wallet-windows-autoupdate-20261029/five-class-s22-origin-delivery.json)。

最终 origin 升级只重新传输各渠道 feed 两文件、engine 三文件，共 10 个输入、3,929,486 bytes，
复核两个原游戏组件的八份元数据大小和 SHA-256。它沿用此前的完整游戏载荷验收，
没有宣称本次引擎升级重新 hash 全游戏。见
[固定准备记录](generated/player-qa/dubhe-wallet-windows-autoupdate-20261029/engine-s23-s24-preparation.json)、
[本地字节绑定](generated/player-qa/dubhe-wallet-windows-autoupdate-20261029/engine-s23-s24-local-binding.json)和
[服务器发布回执](generated/player-qa/dubhe-wallet-windows-autoupdate-20261029/engine-s23-s24-origin.json)。
交换前两份旧 feed 与 Caddy 配置备份于服务器私有目录
`/var/backups/mir2-client-updates/engine2324-20261028T203449Z-4f2be0c54a`。
固定脚本先交换目标 24，再交换桥接 23；签名元数据、公开读取、健康检查和持久化成功报告均通过，
`caddyChanged=false`。其回滚范围包含成功报告写入及 fsync，23 项合成准入与故障测试通过，见
[发布工具验证](generated/player-qa/dubhe-wallet-windows-autoupdate-20261029/engine-only-origin-tool-verification.json)。
实际执行的[固定 helper](generated/player-qa/dubhe-wallet-windows-autoupdate-20261029/engine-s23-s24-origin-helper.py)
SHA-256 为 `8F9E0B7F9842729FDC8BD7E999D2B22C5A493421D06403FFD6CE5F205D409409`，
经外层 SHA guard 复制为 root 所有的受保护代码后执行。CMS 密码学验签在工作站准入阶段完成；
服务器绑定获准字节及其大小、SHA，不假称服务器重新完成 CMS 验签。

## 数据与运行服务

本轮只发布 Windows 下载内容及所需静态路由、Cloudflare 下载配置，不写账号或角色数据库，
不重启测试网关、钱包登录服务或同机另一环境的网关，不启动 Docker。
账号、角色、存档和既有登录方式继续沿用原测试服数据。
2026-10-28 发布记录中的账号、角色数量是上次部署时的快照，不作为本轮重新查询的计数。
在 `2026-10-28T21:26:13Z` 的[最终服务复核](generated/player-qa/dubhe-wallet-windows-autoupdate-20261029/final-services.json)中，
测试网关、同机另一环境网关、钱包登录和 Caddy 的原 PID 均保持；网关和登录服务仍为 `00ca0f93f`，
测试服继续使用 `platinum_176`，健康检查通过，`databaseOperationsPerformed=false`、`dockerStarted=false`。

首次钱包登录使用独立的 canonical 钱包账号；不会按昵称或钱包地址猜测并绑定既有密码账号。
扫码后看到空角色列表不表示原账号角色被删除。账号创建及登录流程见
[Windows Dubhe Wallet 登录说明](DUBHE-WALLET-WINDOWS-LOGIN.zh-CN.md)。

## 发布与验收记录

| 项目 | 当前结果 | 证据 / 边界 |
| --- | --- | --- |
| Candidate28 完整严格验收、签名及最终字节绑定 | 通过 | 上述两份构建证据；最终绑定时间 `2026-10-28T18:36:10Z` |
| 职业策略、登录相邻回归与离屏画面 | 通过 | 上述职业策略证据；未连接真实服务器 |
| 中间 sequence 22 origin 安装 | 通过；已被最终引擎渠道替换 | [origin 发布回执](generated/player-qa/dubhe-wallet-windows-autoupdate-20261029/five-class-s22-origin.json)，退出码 0 |
| 中间 sequence 21 / 22 R2 暂存 | 通过 | [21 暂存](generated/player-qa/dubhe-wallet-windows-autoupdate-20261029/legacy-s21-stage.json)、[22 暂存](generated/player-qa/dubhe-wallet-windows-autoupdate-20261029/five-class-s22-stage.json)；不是最终 23 / 24 暂存回执 |
| 中间 sequence 22 CDN 指针及别名 | 曾通过；现已被最终 24 替换 | [22 提升回执](generated/player-qa/dubhe-wallet-windows-autoupdate-20261029/five-class-s22-promote.json)，时间 `2026-10-28T19:22:10Z` |
| 最终 sequence 23 / 24 origin 引擎升级 | 通过 | [发布回执](generated/player-qa/dubhe-wallet-windows-autoupdate-20261029/engine-s23-s24-origin.json)，退出码 0 / `passed=true`；八份游戏元数据复核，未重新 hash 全游戏载荷，未改 Caddy |
| 最终 sequence 23 / 24 R2 暂存 | 39 / 39、42 / 42 通过 | [23 暂存](generated/player-qa/dubhe-wallet-windows-autoupdate-20261029/legacy-s23-stage-assets.json)、[24 暂存](generated/player-qa/dubhe-wallet-windows-autoupdate-20261029/five-class-s24-stage-assets.json) |
| 最终 sequence 24 CDN 提升 | 通过 | [24 提升回执](generated/player-qa/dubhe-wallet-windows-autoupdate-20261029/five-class-s24-promote-assets.json)，`2026-10-28T20:48:46Z`；exact expectedCurrent 22，`pointerChanged=true`、`aliasesVerified=true` |
| 最终 sequence 23 CDN 桥接提升 | 通过 | [23 提升回执](generated/player-qa/dubhe-wallet-windows-autoupdate-20261029/legacy-s23-promote-assets.json)，`2026-10-28T20:51:29Z`；exact expectedCurrent 20，目标 24 已激活且完整固定对象集通过 |
| 真实原版 b439 启动器、原 9213 引擎正常桥接 23 | 通过 | [真实第一次桥接记录](generated/player-qa/dubhe-wallet-windows-autoupdate-20261029/real-old9213-public-bridge-s23.json)：20 → 23、新引擎 1 文件、游戏改动 0；实际 R25 存活 10.94 秒，由原更新器自然接受健康状态，`healthSimulated=false`；未提交登录或账号操作 |
| 真实原版 b439 启动器第二次 `--check-only` 安装 24 | 通过 | [真实安装及全载荷校验](generated/player-qa/dubhe-wallet-windows-autoupdate-20261029/real-legacy-launcher-public-s24.json)、[两次运行准入](generated/player-qa/dubhe-wallet-windows-autoupdate-20261029/real-public23-24-e2e-admission.json)；实际 C824、24 激活、255,069 文件全 SHA、个人文件与日志保留；游戏尚未启动 |
| CDN / origin 最终地址、签名元数据、配置及 Range 复核 | 24 项通过 | [两个域名的公开复核](generated/player-qa/dubhe-wallet-windows-autoupdate-20261029/public-release-verification.json)：23 / 24 feed 签名字节、GET / HEAD / no-store、TOML、Range、方法及私有路径隔离、四处新引擎完整 SHA |
| 临时发布 Worker、路由及 BIC 规则清理 | 通过 | [Cloudflare 清理与读回](generated/player-qa/dubhe-wallet-windows-autoupdate-20261029/cloudflare-publication-cleanup.json)：三次 DELETE 200，随后确认三项均缺席；清理时正式 reader、主路由、长期 BIC 规则 version 2 均保持原值 |
| 本机临时传输凭据清理 | 通过 | [精确三文件清理](generated/player-qa/dubhe-wallet-windows-autoupdate-20261029/private-transport-cleanup.json)，`secretValuesRecorded=false` |
| 临时入口清理后的 CDN / origin 公开复核 | 24 项再次通过 | [清理后公开复核](generated/player-qa/dubhe-wallet-windows-autoupdate-20261029/public-release-after-cleanup-verification.json)，`2026-10-28T21:30:13Z`，最终 23 / 24 仍正常提供下载 |
| Candidate28 首次正常启动健康确认、可见界面与进入游戏 | 待人工验收 | 第二次 `--check-only` 不启动 Candidate28，也不接受其首次游戏健康状态；不能用第一次旧 R25 健康确认代替 |
| Dubhe Wallet 手机真人扫码签名与进入游戏 | 待人工验收 | 自动签名校验、relay pending 和无效签名测试不能代替真人钱包授权 |

[此前本地桥接测试](generated/player-qa/dubhe-wallet-windows-autoupdate-20261029/legacy-bridge-source-e2e.json)
曾模拟 sequence 21 的健康确认，不能作为真实游戏健康运行证据。
最终 C824 检查已使用真实旧启动器执行第二次 `--check-only`，完成实际更新引擎、安装事务和全部载荷验证；
此模式不启动游戏，也不会模拟接受 sequence 24 的首次游戏健康确认。
构建报告中的 `visualAccepted=false` 应保持原值。

[临时发布工具验证](generated/player-qa/dubhe-wallet-windows-autoupdate-20261029/publisher-tool-verification.json)
记录 20 项 Node 安全测试、7 项 Python 传输测试及 3 项真实 workerd 运行时测试通过。
该文件记录的是部署前工具检查，实际公网发布结果以上述渠道提升回执及最终公开地址复核为准。
最终 23 / 24 的固定计划适配与验证见
[最终发布工具验证](generated/player-qa/dubhe-wallet-windows-autoupdate-20261029/s24-tool-verification.json)。
