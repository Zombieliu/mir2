# 2026-10-30 统一测试服与公网更新发布证据

本目录保存实际远程发布、公网读回和 Windows 迁移的脱敏证据；不是只在本机生成的发布计划。当前结论见[发布记录](../../UNIFIED-PLAYTEST-ROLLOUT-20261030.zh-CN.md)。58 份证据的大小和 SHA256 见 [EVIDENCE-INDEX.json](EVIDENCE-INDEX.json)。私钥、原始环境文件、数据库备份和原始资产账本均未复制入库。

## 已生效的组件

| 组件 | 固定版本 | 实际证明 |
| --- | --- | --- |
| Gateway | `e824e520b902e5fc616dc8fbb806b3a4363a60ba` | [CI 构建及 ZIP 校验](CI-ARTIFACT-VERIFICATION.json)、[真实 PG/Redis 主程序启动](gateway-postgres-startup.json)、[切服](ACTIVATE-ACTUAL.json) |
| Wallet / Node 服务及链驱动源码 | Wallet / runtime `600444277cdee49055d9c2cb9df1b0a71246e1c9`；复用已固定 Node22 | [钱包真实 ZIP 校验](WALLET-ACTUAL-ZIP-VERIFICATION.json)、[Linux 依赖](linux-dependency-summary.remote.json)、[官方测试网只读预检](remote-read-only-preflight-v2-repeat.json) |
| Windows Game | `1875c6c20e1285ffefc45ef6c21eab33e26fc536` | [实际公网迁移](PUBLIC-MIGRATION-SUMMARY.json)、[33 包及关键文件复核](PUBLIC-MIGRATION-FINAL-HASHES.json) |
| Windows Engine | `77a2007a449d1f437a8d411bfa41f105be992fcf` | 同上；桥接下载仅更新 Engine，新 Game 在第二轮启动后生效 |

e824 修复了先前 600 release 在真实 PostgreSQL 启动时触发的嵌套 Tokio runtime panic。失败的 600 Gateway 未激活到公网；Wallet 和 JS 链驱动作为未变的独立组件保留 600 身份。不能把上述不同构建身份改记为同一个 HEAD。

## 原服数据与切换

- [克隆普通账号验收](VERIFY-V2-ACTUAL-SUMMARY.json)：创建、建角、进入游戏、密码恢复、重载及存档投影通过；克隆运行期间原服 46 张非时钟表完整摘要保持，两张时钟表的全部资产投影保持。严格[时钟审计](clock-audit.remote.json)没有跳过整张表。
- [停服与一致备份](STOP-AND-BACKUP-ACTUAL.json)：原 TEST Gateway SIGINT 正常退出 `0`，没有强杀；离线 PG dump 与原身份密钥/环境保留在服务器受限备份目录。原 state/recovery 全部 8 文件、7 目录、475,982,279 字节逐 SHA 比对归档一致。
- [切服后数据库](POST-CUTOVER-DATABASE-RETENTION.json)：119 账号、35 存档和 35 投影保留；45 张原表完整摘要一致。其余差异为迁移记录 16→19 及两张时钟表，新增三张 USDC 表，原表未删除。另有[重启后领地完整资产核对](POST-RESTART-CLOCK-ASSET-RETENTION.json)，正常停服释放旧租约、重启取得新租约，没有改变其余领地 JSON 资产。
- [最后健康状态](FINAL-REMOTE-HEALTH.json)：Gateway958483、Wallet958517 活跃；生产 Gateway3855184 与 Caddy2107428 保持。原 PostgreSQL/Redis、身份认证、`platinum_176`、月卡1000点券保留。容量上限仍为66 WS/51 active，未据此声称500 CCU通过。

需要回退时，先排空并正常停止 TEST 服务，再由操作者按兼容性审阅同时恢复对应 PG、state/recovery、身份/MAC密钥及配置/程序；不得只把旧 binary 指向已迁移的新库。本轮没有执行自动降级、SQL glob、Docker 操作或生产服务重启。

## 公网发布与 Windows 实测

[源站原子切换](ORIGIN-PROMOTE-ACTUAL.json)按 packed→five-class→legacy 执行，旧目录/符号链接均保留在受限备份。CDN 三次[发布汇总](R2-PROMOTE-ALL-ACTUAL.json)及 [packed](promote-packed-v1.json)、[five-class](promote-five-class-v1.json)、[legacy](promote-legacy-v1.json)真实回执齐全。

[公网读回](PUBLIC-FEEDS-VERIFIED.json)覆盖源站/CDN 三频道共12次 GET、6份 detached CMS，固定 RSA 公钥与 feed/signature SHA 全部匹配，响应均为 `no-store`：packed `2026102912`、five-class `32`、legacy `33`。实际读回字节保留在 [public-readback/](public-readback/)。

[安装包验证](PUBLIC-INSTALLER-RANGE-VERIFIED.json)确认源站完整文件 SHA、1,024,372,943字节、公开 HEAD200及首尾 Range206 一致；不声称完成了整份安装包的公网 HTTP 下载，也不声称有发布者 Authenticode 签名。

[真实 Windows 两轮迁移](PUBLIC-MIGRATION-SUMMARY.json)从隔离 Wallet28/C824/seed24 出发，经生产内置公网 URL 下载桥接 Engine77，旧 Game 正常启动/退出，再下载33包、实际运行1875 Game并达到生产 `Healthy`，两次退出均为0。旧 Game、用户标记、三份个人偏好未变，测试进程已退出。独立重新计算了33包合计1,069,032,581字节及关键文件全部 SHA。

摘要中的 `launchStarted=false` 是生产选择器的启动中标记：引擎在确认Healthy时清除此标记，不代表没有启动。实际[Healthy快照](windows/public-packed-live-healthy.json)、PID和正常退出日志分别保留，原始回执未重写。

截图：[旧引擎扫描](screenshots/01-old-c824-bridge-scanning.jpg)、[桥接后旧 Game](screenshots/02-wallet28-bridge-prelogin.jpg)、[新 Game Healthy](screenshots/03-public-packed2912-healthy-prelogin.jpg)、[正常退出前](screenshots/04-public-packed2912-before-normal-close.jpg)。生产日志和观察记录见 [windows/](windows/)。

本轮不包含登录、手机签名、PG上真实充值/求购交付、崩溃回滚或人工 Accepted。Ready握手及存活门禁由固定生产引擎进入Healthy分支证明，未抓取精确Ready包或计时。此次隔离用户继承English偏好，不作为新增简体中文界面验收。既有语言页脚P2问题仍保留。

## 测试计费与权限清理

[测试配置](configured-v3.remote.json)、[测试价格映射](test-offers-overlay.remote.json)保留现有法币 TEST 套餐比例，将其映射为明确标记的 USDC TEST 档位；不是批准正式价格。原月卡保持独立。USDC发行方总 Gas 预约上限从100m提高为500m Mist，原两笔合计100m预约未清除，[实际预算回执](FINAL-TESTNET-BUDGET-SUMMARY.json)保留前后 SHA；上限、预约和实际 Gas 花费是不同概念。

[本地写入者交接](final-local-handoff.json)与[独立复核](handoff-postcheck.public.json)保留两个旧本地 QA runner 的停止证据：它们属于精确进程终止，不能冒充公服的正常 SIGINT 停服。其私有账本、存档和14文件备份保留，不得重新同时启动旧写入者。

首次远程链只读检查失败及诊断分别保留为 [v1](remote-read-only-preflight.json)、[v2](remote-read-only-preflight-v2.json)；[随后重复检查](remote-read-only-preflight-v2-repeat.json)求购和USDC均通过，账本未变、签名和提交均未调用。最初失败的根因没有确证，不推测为已修复的代码缺陷。

[公开服务检查](PUBLIC-BACKEND-CHECKS.json)确认新版 game/auth HTTPS健康；USDC pay/pay.js 为200且no-store，专用billing域其余探测路径404。[Caddy热重载](CADDY-ACTIVATED-ACTUAL.json)保留原operator环境。

[Cloudflare清理](CLOUDFLARE-CLEANUP-ACTUAL.json)确认临时发布 Worker/secret binding 删除回执及临时路由/BIC规则撤除读回，永久只读下载路由/公开下载规则保留。[本机临时配置清理](PUBLISHER-PRIVATE-CONFIG-CLEANUP.json)如实记录：自动审批返回 `blocked by policy`，未给具体原因，删除未执行；失效凭据副本仍在受限目录。不得将其记为删除成功或重启已撤销的发布服务。

最后检查服务器可用磁盘649,732,096字节，约620MiB；旧备份和旧发布未清除，需要另行扩容或人工审阅清理。所有真实资金、主网和生产收费保持关闭。
