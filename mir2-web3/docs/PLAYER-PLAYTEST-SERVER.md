# 独立玩家内测服部署

本页和 `infra/playtest/` 是待审核的部署材料，不表示服务器已经上线。
目标地址为 `wss://165.154.65.136.sslip.io/playtest/ws`；使用同一台主机的
既有 TLS 入口，为本轮 Windows 包增加独立测试世界。Gateway 在本进程内
运行共享 Zone，不启动第二个 `zone_host`，不接入现有验证节点。

## 隔离边界

| 项目 | 测试服 |
| --- | --- |
| 系统账号 | `mir2-playtest`，无交互登录、无 sudo |
| Gateway 单元 | `mir2-playtest.service` |
| Gateway HTTP / TCP | `127.0.0.1:7210` / `127.0.0.1:7200` |
| Redis | 独立 `mir2-playtest-redis.service`，`127.0.0.1:6381` |
| PostgreSQL | 独立数据库、LOGIN role：`mir2_playtest` |
| 版本目录 | `/opt/mir2-playtest/releases/<完整源码 SHA>/` |
| 存档及恢复日志 | `/var/lib/mir2-playtest/state`、`/var/lib/mir2-playtest/recovery` |
| 私密配置 | `/etc/mir2-playtest/gateway.env`，root:root，0600 |
| 公网路由 | 仅 `/playtest/ws`、`/playtest/health` |

不改动 `/opt/mir2/gateway/current`、`/etc/mir2/gateway.env`、旧 `mir2`
数据库、现有 Zone Host、验证节点或玩家存档。不使用生产安装器的目录替换
来实现此隔离，也不放宽其签名、pin、归档提取和激活规则。

现主机总内存 8 GB、无 swap，部署前只剩约 2.3 GB available。模板给新
Gateway 设置 `MemoryHigh=1400M`、`MemoryMax=1600M`，Redis 上限 256M，
并降低两者 CPU/IO 调度权重。进程可能因超过上限而终止；这用于限制对旧
服务的影响，不构成测试服稳定性证明。不要在该主机编译大规模 Rust 工程。
先做双账号验证，再逐步加到最多 15 个活动玩家；30 个 WS 连接和 15 个
重连租约只是准入上限。

## 发布前材料

1. 使用已推送、工作区干净的完整源码 SHA，在认证 Linux CI/构建机完成
   `mir2-gateway` release 构建。既有 `package-gateway-release.sh` 要求
   Linux、非 root 发布用户及 `MIR2_RELEASE_PUBLISHER_UID=$(id -u)`。
   保存运行 ID、源码 SHA、归档 SHA-256、二进制 SHA-256、构建日志。
2. 从认证的 CI 运行下载对应 artifact，核对运行所属仓库、工作流、提交和
   artifact 哈希。相邻 `.sha256` 文件本身不是发布授权。既有 Gateway
   归档仅有 `mir2-gateway`、`zone_host`、`RELEASE.json`、`README.txt`。
   操作员在非 root 的私有暂存目录检查成员、类型、大小及哈希；不能把
   任意归档直接以 root 提取到版本目录。这里只安装核验后的 Gateway，
   不执行归档脚本、不安装或启动 `zone_host`。
3. 准备相同源码版本的 `apps/web/lib/generated/crystal-map-pack`。这些是
   实际 `.map.gz` 文件，放入版本目录的 `crystal-map-pack/` 并核对完整清单。
   2026-09-28 审计为 1,620 文件、42,111,309 字节；新版本以其清单为准。
   配置 `MIR2_CRYSTAL_MAP_PACK`，不依赖编译机路径或开发目录链接。
4. 渲染 `mir2-playtest.service.example` 和 `gateway.env.example` 的全部
   `REQUIRED_*` 值。`ExecStart`、地图目录、`MIR2_DEPLOY_REVISION` 必须引用
   同一完整 SHA。保留占位值的材料不可激活。

## 首次部署顺序

以下步骤由有权限的操作员在审阅具体 artifact 和配置后执行，模板不会自动
登录服务器、创建数据库、替换配置或重启服务。

1. 记录旧服务的 `/health`、运行版本、进程、端口、内存和磁盘基线；备份
   即将增补的 Caddyfile。确认 7200、7210、6381 空闲。创建无登录的
   `mir2-playtest` 系统账号和独立目录：版本及配置父目录 root 所有，只有
   `state`、`recovery`、`redis` 三个数据子目录归该服务账号所有且 0700。
2. 创建全新的 PostgreSQL role/database `mir2_playtest`。role 不得具有
   superuser、createdb、createrole 或 replication 权限；密码通过私密
   输入设置，不放进命令行、提交、构建日志或玩家包。数据库只授予该 role
   必要权限，收回 PUBLIC 对新库的默认访问；核对本机认证规则和该 role
   对其他库的权限。不要导入旧玩家数据库。首次启动可能为新库建表，
   因此必须先核验两条数据库 URL 都指向 `mir2_playtest`。
3. 生成独立的数据库密码、passkey secret、identity session secret、
   identity recovery pepper、保存恢复 MAC key。后三类身份密钥至少
   32 个字符，MAC key 必须为 64 位十六进制随机值。两个数据库 URL
   使用同一新库凭据，URL 特殊字符必须编码；其他用途的密钥不能复用。
   将渲染的 env 以 root:root 0600 保存并备份。保留恢复日志期间不得
   轮换 MAC key。不要复制旧服务 env 或开启开发密钥选项。
4. 安装审核过的版本文件、Redis 配置和两个新 systemd 单元。Gateway
   使用 `staging`、商业身份、`platinum_176`、`newcomer-v2`、固定白天，
   PostgreSQL/Redis 必须可用；Channel identity 也显式要求 PostgreSQL。
   单元去除 Gate15 validator 和开发绕过变量，不接触旧单元。
5. 先运行 `systemd-analyze verify` 检查渲染后的两个单元，再
   `systemctl daemon-reload`，启动新 Redis，检查其仅监听 loopback6381。
   `redis-cli -h 127.0.0.1 -p 6381 PING` 应得到 `PONG`。
   再启动新 Gateway，检查 7200/7210 仅监听 loopback、日志无恢复错误。
6. 本机 `http://127.0.0.1:7210/health` 必须通过下节检查，才将
   `Caddyfile.routes` 导入现有 sslip.io 站点块。它只代理两个玩家路径，
   对 `/playtest/admin/...`、`/playtest/onchain/...` 等返回 404。
   `caddy validate --config /etc/caddy/Caddyfile --adapter caddyfile`
   通过后只 reload Caddy，不重启旧 Gateway。Caddy 默认会在 reload 时
   关闭已有代理 WebSocket；操作前确认旧服无活动连接，或先验证现配置
   的连接保留行为。参见 [Caddy streaming 文档](https://caddyserver.com/docs/caddyfile/directives/reverse_proxy#streaming)。
   失败时恢复之前的 Caddyfile。
7. 从公网验证 `/playtest/health` 和真实 WSS，重新核对旧 `/health` 的基线。
   确认通过后才分发写入真实 WSS 地址的独立 Windows ZIP。

当前 Redis 客户端没有 AUTH/SELECT，实现里 namespace 也固定。因此这里
使用独立 Redis 进程；`redis://127.0.0.1:6379/1` 不能代替隔离，带密码
URL 也不能冒充受支持的 AUTH。本方案信任同机进程，只允许 loopback
访问 6381，不适用于不可信用户共享的主机。不要复用旧 Redis 或关闭
现有 Redis 的认证。缓存满时采用 `noeviction`，避免淘汰会话租约和撤销
记录；出现缓存错误应停止扩大测试人数。

## 验收与观察

健康接口不能只看 `ok=true`。要求 `revision` 等于已核验的服务器完整 SHA；
客户端及服务器各自记录源码和二进制哈希，并用这套具体组合做联网验收。
`http`/`ws` 为 ready，`identity_backend=postgres`，`channel_identity`
为 PostgreSQL 且 durable，`session_cache.backend=redis`、configured/
healthy 为 true，容量为预期上限；单进程世界的 `gate15` 应为空。
启动验证还应检查 process cgroup、监听地址、地图包可读性和新库归属。

使用两个独立普通账号，经客户端注册/登录/建角，验证互见和移动广播、共同
打怪的共享血量、入口换图、断线重连、正常退出后重新登录的存档。密码至少
10 字符，不能与账号相同；不能给玩家 `demo/demo` 或在包里预置账号。
未认证 StartGame/NewCharacter/DeleteCharacter 和调试/QA 命令应被拒绝。
原生 TLS 必须使用系统信任根校验证书和域名；Origin 应为
`https://165.154.65.136.sslip.io`，不要放宽服务端 allowlist。

记录 `/health` 延迟、RSS/cgroup memory、正常动作延迟、地图转换和保存错误。
每次扩容前重测；内存触顶、频繁重启、保存/恢复失败、旧服务健康变差时先
停止测试服新增连接并保留诊断。15 人上限不是性能通过结论。
“邀请试玩”目前指小范围分发安排，端点没有邀请码注册限制；如需只允许
指定人员接入，应另配置经核验的入口 IP/VPN 访问控制，不使用原生客户端
无法回答的 HTTP Basic Auth，也不把隐藏 URL 当访问认证。

## 更新、备份与回滚

更新前通知玩家正常退出。采用已验证能保留现有连接的策略，暂时关闭
**测试服** 的新 `/playtest/ws` 升级入口并保留 health；未验证该策略时，
先等现有连接正常退出，再变更入口，不能假定 Caddy reload 保留 WebSocket。
确认活动玩家、WS 连接及重连租约都为 0 后，再停止 `mir2-playtest`。
SIGINT 让服务器完成自身停止逻辑，
不代替正常退出和存档核对。超时强制终止或 OOM 后必须先检查恢复日志。

在停服状态生成新库的 `pg_dump` 备份，保存对应 `gateway.env`、MAC key、
完整 recovery/state 目录、版本哈希和单元文件；备份限管理员访问，并
至少验证归档可读和恢复所需文件齐全。数据库与 MAC/journal 是一组，不能
只备份其中一个。不要在日志中打印 env 或恢复凭据。

安装新 SHA 的独立版本目录，先验证数据格式兼容，再更新测试服单元和
非密钥 env 中的 revision/地图路径。保留上一版本、新旧备份和稳定密钥；
重新做本机、公网和普通账号保存验证后恢复测试入口。

回滚只操作这套测试服：先排空并停止 Gateway，检查旧二进制与当前新库/
恢复日志是否兼容，再恢复上一单元和版本引用。若存在不兼容 schema/save
变化，保持停服，使用整套一致备份在独立恢复库中验证；不要直接将旧程序
指向未知的新 schema，也不要自动覆盖玩家进展或生产库。Caddy 路由异常
可单独撤回新增片段并 reload，旧 Gateway 路由继续使用原配置。

模板尚未在目标主机通过 systemd/Caddy 解析、Linux安全门及实际玩家验收。
这些结果必须由部署记录另行补充。

## 9 月 29 日准备阶段校正

- `/etc/mir2-playtest` 用 root:mir2-playtest 0710，仅提供路径遍历；Redis
  配置用 root:mir2-playtest 0640。网关 env 与私密 bootstrap 保持
  root:root 0600，不能为了 Redis 可读而公开全部密钥。
- 此专用回环监听器由 Caddy 强制将 `CF-Connecting-IP` 改写为真实 TCP
  来源，再允许网关读取。客户端传入的同名头被覆盖；不允许把 7210 暴露
  公网。这样注册限额按真实来源计算，避免所有玩家共用代理的 127.0.0.1。
  User-Agent 也不能独自代表设备，否则同版本原生客户端会共用全服限额。
- 服务去除外部 Zone、拓扑和环境优先级变量，避免接入旧服世界。
- Redis 已在目标主机通过 systemd 检查并启动，6381 仅监听 127.0.0.1，
  PONG、无重启。Linux 非 root 发布安全检查通过。Gateway、Caddy 新路由及
  公网玩家测试仍待后续部署记录确认。
