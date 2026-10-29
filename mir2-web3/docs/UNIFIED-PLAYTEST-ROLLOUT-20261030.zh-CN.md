# 统一版本测试服与公开测试更新渠道（2026-10-30）

测试服后端与三个公开测试更新渠道已切换，origin/CDN读回及签名验证通过。Game1875与Engine77沿用冻结字节，Gateway修复版本为e824；本轮没有重新构建Game。Windows实际完成Wallet28/C824→Engine77桥接→packed2026102912，两次登录前启动与正常关闭通过。手机真人签名、原生PG上的新采购/充值真实链交付及最终人工Accepted不在已通过范围。

本轮统一证据见[发布证据索引](generated/unified-playtest-rollout-20261030/README.md)。历史本地集成及资源验收继续见[统一集成记录](UNIFIED-WALLET-RECHARGE-20261029.zh-CN.md)，不重写旧generated证据。

## 版本与服务

| 组件 | 来源与观察结果 |
| --- | --- |
| Gateway | `e824e520b902e5fc616dc8fbb806b3a4363a60ba`，Linux EXE SHA256 `010c6627c9a18663eb56a5618c220f46f6051b88e3c944e621166072d9aef2ea`；切换PID958483 |
| 钱包登录 | 组件源码 `600444277cdee49055d9c2cb9df1b0a71246e1c9`；切换PID958517 |
| Game | `1875c6c20e1285ffefc45ef6c21eab33e26fc536`；VERSION SHA256 `51afe182412b0cd7dabeb23228f3cce354d82db9694062fa40fab1166397308a` |
| Engine | `77a2007a449d1f437a8d411bfa41f105be992fcf`；实际发布沿用已验证字节 |
| Node/链驱动 | Node22.23.4，SHA256 `94efd062ed066c1a0b332e18249859c905e920eeada5df55d24c99b919629a90`；10个源/锁文件仍固定独立组件600 |

原600 Gateway在隔离启动时暴露nested Tokio runtime panic，从未切换到公服。e824修复并增加真实原生PG主程序启动门禁；JS/runtime组件600的复用不表示仍运行旧Gateway二进制。

[CI40936713249](https://github.com/Zombieliu/mir2/actions/runs/40936713249)对e824的7项发布门禁通过：schema_union、monthly_card_postgres、billing_postgres、activities_postgres、linux-release-security、gateway-package、gateway-postgres-startup。真实release Gateway子进程使用原生隔离PG/Redis，410ms，正常退出，包内二进制摘要匹配；该启动样例关闭采购/USDC新发布且未配置链驱动，不能替代真实交付。CI有2个目标构建成功、3个复用作业skipped，不能写成7个job全部执行。全程未启动Docker。

## 切换与数据保留

公服先排空并正常停止旧测试Gateway，`forceKill=false`；同机另一环境PID3855184未动。停服后生成一致PG备份，并核对canonical state/recovery归档：8文件、7目录、475,982,279bytes，逐项内容完整匹配。原身份、钱包鉴权与存档恢复密钥、PG/Redis配置保留。

- 离线PG dump SHA256：`634de5180c6c145a3392b65759d59eae768245a42525fb26dec8f81a02e55d84`。
- state/recovery归档SHA256：`6904f061fb90634e2b0720a20b0ef5af1c6de4faa2e0c03d7408a99d4069b9bc`。
- 切换后119账号、35角色状态、35角色存档；原48表无缺失，其中45表摘要完全一致。
- 另外3张旧表分别为迁移登记16→19、`shared_conquests`及`shared_guild_clock`后台时钟变化，单独记录；不能称所有48表或切后46表完全不变。新增3张USDC表后共51表。仅执行注册迁移，未glob执行SQL。

e824克隆库普通身份/保存流程及严格clock-only保留谓词通过后才准入切换。本轮没有把本地File账号库导入公服，也没有重置既有账号。上述119/35为本轮切换复核值；10月28日同数字的历史快照保持独立。

重启后另以固定一致离线dump核对完整conquest非clock JSON，包括金币等资产字段：原lease正常释放为null、新lease结构合法、clock generation/database version单调，其他JSON与guild singleton保留，未输出行值或写库。见[切后表摘要](generated/unified-playtest-rollout-20261030/POST-CUTOVER-DATABASE-RETENTION.json)及[重启后资产/clock核对](generated/unified-playtest-rollout-20261030/POST-RESTART-CLOCK-ASSET-RETENTION.json)。

另有两个旧**本地QA**Gateway61784/84280在14文件完整私密备份、程序/端口与零业务核验后，经授权非优雅停止；两次复核账本、账号库和回收文件不变，无pending/unknown/unclaimed，p01未动。这与公服正常停机是两件事。实际交接回执SHA256为`ce29272d9a60166a51a7db485be4a83e06dc754f220299eaae16470b64a202c0`；远端接管相同签名者/账本后，不得再开这两个旧runner。

## 公网与测试链配置

Caddy按白名单激活新增支付页面路由，原PID2107428保留。公网检查结果：

| 地址 | 实际结果 |
| --- | --- |
| [测试服健康](https://165.154.65.136.sslip.io/playtest/health) | HTTP200、ready，源码e824 |
| [钱包服务健康](https://auth-playtest.165.154.65.136.sslip.io/health) | HTTP200、ready、no-store，组件600 |
| [USDC测试支付页](https://billing-playtest.165.154.65.136.sslip.io/v1/billing/sui-usdc/pay)及`pay.js` | HTTP200、no-store；具体订单仍需授权 |
| billing域名的`/`和`/health` | HTTP404，未开放任意Gateway路径 |

原`platinum_176`、身份策略与月卡有效价1000保留。月卡来自原ordered `99-wallet` EnvironmentFile；base文件本身没有该字段，原始完整SHA256固定为`cbc3a385c14e016c76bb6e55512582bf89324cece4d4a3b4bcdeb94b0c540378`，没有修改base来填1000。新增testnet env/keys追加到原配置层之后，不移除旧overlay。

TEST USDC四档由原有效USD1/5/10/20套餐逐项派生为100/500/1000/2000点券，ID加`usdc-test-`前缀，标签明确TEST USDC，规则版本1。原Stripe套餐和月卡不变；这是已有比例的TEST新通道，未批准正式生产价格。采购服务费生产计费关闭，链操作仅Testnet `4c78adac`，不部署主网、不用真实资金或新生产密钥。

Linux锁定依赖与7827文件闭包、低权限SDK导入通过。USDC使用npm10.9.3 `ci --legacy-peer-deps --omit=dev --ignore-scripts`精确复现21依赖，原锁/SDK不变；user/global npmrc使用不同空文件。最终真实只读预检两项均通过：采购readClock与USDC configureValidate，publishingEnabled=true；签名/交易为0，账本未变。此前两次采购driverRejected保留，根因未确认，不归因于RPC、依赖或其他猜测。

USDC issuer lifetime cap为500mMIST，即0.5 **Test SUI的预约上限，不是实际Gas支出**。原2笔50m预约保留，额外预约最多8笔，不保证8单成功；失败/重试/重启不重置账本。采购签名者预算独立保留。[最终账本摘要](generated/unified-playtest-rollout-20261030/FINAL-TESTNET-BUDGET-SUMMARY.json)绑定扩额前后哈希及仅maximum字段改变。实际Gas只依据最终交易effects汇总；本次切换和只读预检没有执行新采购成交或真实充值交易，不能用旧File验收替代公服原生PG交付。

## 公开更新渠道与下载

origin先补齐39个缺失对象及完整安装器，冻结59对象闭包；三套stage通过后按packed→five-class→legacy顺序完成提升，旧origin文件和别名保留在备份中。R2三次CLI提升均exit0、pointerChanged=true。公网origin/CDN共12个GET、6份CMS实际通过固定RSA公钥验证。

| 渠道 | 原sequence | 已发布sequence | 作用 |
| --- | --- | --- | --- |
| packed |31（origin；CDN此前无指针）|2026102912|统一Game1875与重建33packs |
| five-class |24|32|旧Wallet28/C824桥接Engine77，保留旧Game身份 |
| legacy/root |23|33|旧root渠道桥接Engine77 |

[统一r11安装器](https://165.154.65.136.sslip.io/client-updates/installers/Numeron-Legend-of-Rebirth-20261029-unified-r11-Setup.exe)源文件1,024,372,943bytes，SHA256 `387fd7140a1457c3977032c4577ad1914de2f70c8402ff4312faf1a0bc4f647f`。公网HEAD200及首尾两个Range206与源字节一致；未声明完整公网GET，也未声明publisher Authenticode。完整安装器由origin提供，R2现128MiB对象限制不变。

已提升的高sequence不能靠降低feed撤销；保留本地防降级floor、CMS、路径/host门禁。需要回退时先排空、核验旧二进制与当前schema/recovery兼容性，必要时在独立恢复库验证一致备份；不能直接把旧程序指向未知新schema或覆盖玩家进展。已见高sequence的客户端修正发布需更高序列。

## Windows真实迁移观察

实际验收使用隔离目录及真实生产Launcher/Updater，走编译内置公网地址。首次C824完整扫描90.5s，实际桥接32仅更新Engine77，旧Game PID85792正常exit0、旧Game文件保留；第二次实际下载33packs，wire1,271,553,456bytes、27.7s，启动统一Game PID88428并从pending进入生产Healthy状态，随后正常exit0。夹具进程最终为0，三份全局个人设置及隔离userdata marker保持，未手工改floor/selector或源码，也未读取/写入正常D盘安装目录。

关闭后独立全量hash复核33packs，共1,069,032,581bytes及3个关键文件通过。实际Game EXE SHA256为`85879bfeb1b44d4c759e6a338338bb58a383b7701f886d1090a96b6267f00c6f`。[真实迁移摘要](generated/unified-playtest-rollout-20261030/PUBLIC-MIGRATION-SUMMARY.json)与[最终全量哈希](generated/unified-playtest-rollout-20261030/PUBLIC-MIGRATION-FINAL-HASHES.json)记录本轮具体范围。

Healthy分支由固定生产引擎执行，要求真实认证Ready握手及10s存活；未捕获原始Ready pipe包或精确Ready耗时，不能把该条件写成外部精确计时结果。27.7s及wire量仅为本机当时连接的单样本，不是带宽SLA、并发或容量认证。旧seed安装器整包未完整attest；本轮没有执行登录、人物进服、手机签名、充值/采购交易或crash rollback。

迁移摘要的`launchStarted=false`表示生产选择器在Healthy时已清除“启动中”标记，不代表未启动；实际Healthy快照、Game PID及正常退出日志独立保留，原始回执没有修改。

旧Wallet28在隔离profile写入English，最终Game保留该语言；本次不新增简体界面验收，已有Game1875简体发布内容的证据保持独立。r10的seed2026102910高于32/33，仍需统一安装器路径，不能清除floor来假装feed升级。

手机真人扫码签名、普通玩家完整前端验收、原生PG上的新链交付以及最终人工Accepted仍未完成。本次发布不改变500CCU容量未验收或整体Candidate100未达成的状态。

## 清理与材料限制

临时Cloudflare route、Worker及其secret binding、临时BIC规则已取得删除API确认并复查routes/rules；永久下载reader/public BIC规则保留，R2已发布对象未删。本机临时私密发布配置的删除被自动审批拒绝，仍保存在ACL受限目录，不能写成已删除；对应令牌随Worker删除失效。

最终[远程健康复核](generated/unified-playtest-rollout-20261030/FINAL-REMOTE-HEALTH.json)时间为2026-10-29T19:55:38Z：四个服务active，六项当前连接/会话/在途计数均为0，PG/Redis、版本、profile、月卡1000和测试预算检查通过。剩余磁盘649,732,096bytes，约620MiB；继续大包或备份前需扩容或人工审核清理。本轮只压缩已验证且本任务拥有的依赖构建副本，未删除其他环境或旧备份。

配置、账号、私钥、恢复MAC、SSH凭据和私密转移包不进入公开证据。发布与验收范围以本topic及[筛选证据](generated/unified-playtest-rollout-20261030/README.md)为准，保留失败记录与未覆盖项目。
