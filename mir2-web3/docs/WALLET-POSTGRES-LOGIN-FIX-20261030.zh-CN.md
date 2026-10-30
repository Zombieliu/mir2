# Windows 钱包登录 PostgreSQL 持久化修复（2026-10-30）

## 问题与范围

玩家在 Windows 客户端扫码后看到 `obl_…` 账号和“账号服务暂时无法使用，请稍后重试”。
正在运行的客户端配置连接独立测试服 `/playtest/ws` 与 `auth-playtest`，未经过宣传官网重写。
本次仅修复测试网关的钱包账号持久化，不发布 Windows 自动更新，不启动 Docker。

## 根因证据

- 测试服源版本：`e824e520b902e5fc616dc8fbb806b3a4363a60ba`。
- 网关日志：截图对应账号执行登录时 `postgres account upsert failed …: db error`。
- PostgreSQL 于 2026-10-30 17:07:09（UTC+8）记录 `invalid byte sequence for encoding "UTF8": 0x00`，参数 `$2`。
- `config.rs::upsert_account_record` 的 `$2` 为 `password_snapshot`。
- 原 `LOCKED_ACCOUNT_PASSWORD` 含 NUL，既不能写入 TEXT，也不能写入 JSONB。
- 调查时原测试服仍有 119 个账号、35 个角色、35 条存档，截图的新账号尚未创建。

## 修复

源提交：`2decb491856dcd8d9b91aa40c7fca32dcf18101a`，基于当前测试服版本单独构建。

- 新锁定标记不含 NUL，并采用刻意无效的 Argon2 PHC 格式；旧版本也会拒绝它，避免回滚后将标记当成明文密码。
- 验证器明确拒绝新旧标记，已有文件存储记录在钱包登录时迁移。
- 已绑定钱包的普通账号保留原 Argon2 密码；事务内再次检查，避免钱包登录覆盖原密码。
- 无数据库结构迁移，无账号、角色、存档清理。

## 验证与发布记录

[原生 Ubuntu CI](https://github.com/Zombieliu/mir2/actions/runs/41033137982) 执行钱包、Passkey、密码、失败回滚回归及独立 PostgreSQL 测试，随后构建测试网关。
PostgreSQL 测试仅允许显式独立数据库名与随机 schema，先复现旧 NUL 的 SQLSTATE `22021`，再验证创建、持久化、重新加载、钱包登录成功，以及密码登录/改密拒绝锁定标记。

CI 实际通过：wallet 23、passkey 8、password 21、账号创建失败回滚 1；各组均为 0 失败，过滤组之间存在重复用例，不作为唯一用例总数。真实 PostgreSQL 集成测试 1 通过、0 失败、0 忽略。
结果与原始时间行见 [TEST-RESULTS.json](evidence/wallet-postgres-login-20261030/TEST-RESULTS.json)。

不可变 Artifact ID `12968045359`；下载 ZIP 的 SHA256 与 GitHub Artifact API digest 一致，内部源码 SHA 与 CI 一致，网关 SHA256 为 `d082013fa1791933c831cafe44fe1e6c07830a5339c066d33604a3170e88dbbd`。
完整收据见 [VERIFIED-ARTIFACT.json](evidence/wallet-postgres-login-20261030/VERIFIED-ARTIFACT.json)。

测试服已更新至 `2decb4918`，新 PID `1088288`。切换前完成在线备份，SIGINT 正常停止旧服务后再完成离线备份；两份私有备份均通过目录校验。
账号 119、角色 35、存档 35、物品 210、邮件 8，以及迁移记录 19 的整行聚合 SHA256 在切换前后完全一致。
原六份环境文件及已有服务配置字节未改；正式网关、钱包登录服务、Caddy PID 保持不变。没有启动 Docker 或发布 Windows 安装包。
部署证明见 [DEPLOYMENT.json](evidence/wallet-postgres-login-20261030/DEPLOYMENT.json)。

17:51（UTC+8）独立公网检查：测试网关 HTTP 200 且 revision 正确，PostgreSQL 持久化与 Redis 健康；独立钱包登录服务 HTTP 200、ready，仍为 `600444277…`；正确原生 Origin 的 WSS 101 握手通过并立即关闭，没有发送登录或写入账号。
证明见 [PUBLIC-SMOKE.json](evidence/wallet-postgres-login-20261030/PUBLIC-SMOKE.json)。

人工验收尚待玩家在当前 Windows 客户端按 F8，重新扫码并签署新的登录请求，确认进入选角/创建角色界面。服务端自动测试不代替该步骤，客户端无需重新下载安装。

## 回滚边界

只新增本次 `zzzz-wallet-account-fix-20261030.conf` 和独立 revision 环境文件；未覆盖原配置。必要时正常停止新网关、恢复原启动配置并启动旧网关；保留数据与备份，不自动整库回档。
旧版也会将新的无效 PHC 标记判为不可认证，因此不会因回滚将锁定标记当成明文密码。旧版首次创建钱包账号的 NUL 缺陷仍会存在，回滚只用于恢复服务。
