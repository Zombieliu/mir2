# Windows Dubhe Wallet 测试服更新（2026-10-28）

本记录仅覆盖用户授权的代码推送、测试服更新、原账号和角色保留。
测试网关为 `mir2-playtest.service`；同机的 `mir2-gateway.service` 属于另一环境。
本次不启动 Docker，不发布或覆盖 Windows 签名更新渠道。

后续 Windows 签名渠道发布独立记录于
[2026-10-29 自动更新记录](DUBHE-WALLET-WINDOWS-AUTOUPDATE-20261029.zh-CN.md)；
以下内容保留本次服务端部署当日的范围和证据。

## 构建与发布来源

已部署源码：`00ca0f93f0c7e367afec495711e0e04334f4deb8`，分支
`codex/windows-player-journey`。完整分支集成及 Windows 九语客户端验证见
[分支集成记录](BRANCH-INTEGRATION-20261028.zh-CN.md)。

[已通过的无 Docker 发布检查](https://github.com/Zombieliu/mir2/actions/runs/40703807272)
固定 Ubuntu 24.04 与 Rust 1.95，使用原生隔离 PostgreSQL 验证持久化、Linux 保存恢复
安全边界及发布包；登录服务独立使用 Node 22、锁定依赖和实际 standalone HTTP 检查。
原有需要 Docker 的复用作业在本次手动模式下跳过，不发布 R2。
从已认证的 GitHub 构建记录取得归档 SHA-256，再在服务器核对归档及逐文件清单。

## 数据保留

更新前快照：119 个账号、35 个角色、35 条角色存档、210 条角色物品、8 封角色邮件。
原 PostgreSQL 数据库、账号主键、角色主键及恢复目录沿用；不清库、不重建玩家账号。
保留原 passkey、身份会话、身份恢复与存档恢复密钥。

私有备份目录为 `/var/backups/mir2-playtest-wallet-20261028-2100`，包含可列出恢复目录的
PostgreSQL custom dump、配置、旧可执行文件、状态和恢复文件。切换前另做停服后的离线备份。
备份及含密钥文件仅保存在服务器受限目录，不进入 Git。

沿用[测试服隔离边界](PLAYER-PLAYTEST-SERVER.md)：loopback 7210/7200、
`mir2_playtest` 数据库和 Redis 6381；容量仍为 66 个 WS、51 个活动会话和 51 个重连租约。

验证先恢复到独立克隆库，使用隔离 Redis 和私有端口。恢复日志的所有权标记绑定规范路径，
因此 QA 使用私有 mount namespace，将副本挂到原规范路径；不删除或重新签署原标记。
玩家、存档、物品、邮件与身份数据核对逐表摘要；后台世界时钟另行处理。
测试账号只创建在克隆库，普通客户端命令验证退出保存和重新登录恢复位置。

迁移仅新增注册的 `0015_sealed_stone_authority`；保留已有
`0016_shared_conquest_authority`，不执行未注册的 Candidate 物品身份迁移。

## 连接方式

游戏 WebSocket：`wss://165.154.65.136.sslip.io/playtest/ws`。
独立登录 origin：`https://auth-playtest.165.154.65.136.sslip.io`。
登录服务只代理本测试网关的渠道兑换接口，使用原测试服 passkey secret，
不读取账号库、Stripe 或身份恢复密钥。进程只监听 loopback；Caddy 仅开放三个 POST 接口
及 `/health`，其他路径返回 404。

在仓库根目录，用已构建的 Windows 开发版连接测试服；仅加 `-CheckOnly` 可先检查连接：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\mir2-web3\scripts\start-playtest-wallet-client.ps1
```

启动器按网关路径检查 `/playtest/health`，不误查根目录的另一环境。
游戏登录页按 F8，使用手机 Dubhe Wallet 扫码、确认登录签名，再选择或创建角色。
语言沿用 Windows 登录页原九语偏好。

原账号仍使用原登录方式进入原角色。首次钱包登录使用独立的 canonical 钱包账号，
不会根据昵称或钱包地址猜测并绑定既有密码账号；首次扫码看到空角色列表不代表原角色被删除。

## 执行结果

2026-10-28 23:03（Asia/Shanghai）公网及数据库复核通过；具体版本、SHA、备份、
容量与无凭据验证结果见
[部署证据](generated/player-qa/dubhe-wallet-playtest-rollout-20261028/deployment-verification.json)。

- 网关和钱包服务均返回同一源码修订；实际 Linux 网关 SHA-256 为
  `0eeec1db650f5180d3f38cb3ffb95c3734f826df2e275c46e3e1f2ad3bc66bb5`。
- 原 43 张非时钟业务表逐表摘要保持一致：119 个账号、35 个角色和存档、210 条物品、8 封邮件。
  首次摘要使用服务器香港时区；核对原始行未变化后统一为 UTC，原摘要保留，未修改数据库行。
- 克隆库普通客户端完成注册、登录、建角、移动、退出和重新登录；坐标恢复一致，保存版本从 4 增至 7，
  原账号数据未变。临时账号仅存在于克隆库；QA 网关和 Redis 已正常停止。
- 原测试网关以 SIGINT 正常退出，禁用强杀；完成离线备份并验证 dump 可列出后激活新版本。
  原四项鉴权/恢复密钥、两项数据库 URL 和其他既有配置均保留。
- 公网真实 relay 创建与 pending 返回 200；不可信来源返回 403、客户端指定网络返回 400、
  无效 intent 与网关 proof 返回 401；未知路径和 `.env` 返回 404。
  Windows 启动器预检、正确 Origin 的公网 WSS 握手均通过。
- 同机正式网关 PID 和可执行文件未改变。Caddy 只追加独立登录站点；原路由内容逐字节保留。
  独立登录进程使用原生 Node 22.23.4，三个需要容器的复用 CI 作业均跳过；未启动 Docker。

手机钱包实际签名与进入游戏属于人工验收，自动 HTTP 的 pending/无效签名检查不能替代。
