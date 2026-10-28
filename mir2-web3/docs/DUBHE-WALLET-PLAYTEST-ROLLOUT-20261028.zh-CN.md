# Windows 钱包登录测试服更新（2026-10-28）

用户授权推送代码并更新远程测试服，要求保留现有账号和角色，禁止启动 Docker。
本轮以本地集成提交 `18b0ce63666028ebb59c9795c49914b547e07d0f` 为功能基线，
只更新独立玩家测试服及其专用钱包登录服务，不修改正式服或合并玩家账号。

## 初始核验

- 测试服公网 health：`https://165.154.65.136.sslip.io/playtest/health`。
- 核验时版本：`bad3737f265b0547d838382c2bb3592caa5dcb31`；PostgreSQL 身份和渠道绑定、Redis 均报告健康。
- 核验时活动 WebSocket、游戏 Session、重连租约均为 0；实际切换前仍须重新检查。
- 现有容量为 66 个 WS、51 个活动 Session、51 个重连租约；升级应保留实际配置。
- 测试服边界沿用 [测试服运维记录](PLAYER-PLAYTEST-SERVER.md)：`mir2-playtest.service`、
  loopback 7210/7200、数据库 `mir2_playtest`、独立 Redis 6381。

## 数据与上线边界

保留当前数据库、账号及角色主键、渠道绑定、环境密钥、state/recovery、Redis 服务。
排空并停测试 Gateway 后，备份同一时间点的数据库、配置、单元和 state/recovery；
在独立恢复库验证新程序兼容，再激活新版本目录。不能执行本机随机存档启动器、
重建数据库、导入开发角色或将密码账号擅自归到钱包账号。

独立登录服务仅复用已验证的钱包接口实现，使用测试服既有签名密钥和明确的测试
Gateway HTTP 地址。原生登录地址必须为独立 HTTPS origin，提供三条 POST 路径：
`/api/wallet/dubhe/session`、`/api/wallet/dubhe/session/status`、
`/api/channels/session/exchange`。

## 无 Docker 构建

现有月卡、计费自动验收及 Gateway release 的复用任务含 PostgreSQL 容器。
本轮推送提交使用 `[skip ci]` 避免自动启动这些容器，随后手动运行专用的无 Docker
构建分支；它使用原生临时 PostgreSQL、Linux 非 root 安全门及精确源码工件。
保留原 release 默认路径，不能把本次模式作为生产发布的绕过入口。

## 当前状态

服务器访问已核验成功，正在准备无 Docker 构建和受保护备份/升级。
实际工件、版本切换、数据保留检查及验收结果在本记录后续追加，准备状态不代表已部署。
