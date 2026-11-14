# R16 生产版本与扫码核验

2026-11-12（Asia/Shanghai）按用户要求核对并尝试补刷 [Numeron 游戏](https://numeron.obelisk.build/game)。认证 Vercel 读回确认：团队 `obelisk-labs`、项目 `numeron-r16-web`，当前生产为 `dpl_B7mFuzdhnBcgPSrNHR3HBFehqq6y`，状态 READY，源码 `cd6410be749412d608a538e2db442b7e4d519de0`。

`6997f810ed` 玩家反馈修复是该生产源码的祖先；当前分支已提交的 Web 与 packages 文件树和生产源码相同。对同一部署的 promote 请求返回 HTTP 409，明确表示它已经是当前生产版本，因此本轮没有产生新构建或域名切换。

公网 `/version`、Worker 正文长度与 SHA256、11 个启动资源 JSON 的完整正文长度与 SHA256 均通过。普通 Chrome、标准系统网络、全新独立上下文顺序复测简体与繁体：各一次创建、真实二维码矩阵、待确认、正常取消与轮询停止通过；两个浏览器均关闭。

[结构化回执](receipt.json) 仅包含白名单字段，不包含二维码会话、审批链接、密钥或浏览器认证状态。完整原始回执保存在本机 `C:/r16-production-refresh-20261112`。原 62 个 WIP 文件与 38 条状态，以及当前三个字体候选文件均保持字节不变。

本轮未重演实体手机确认或完整游戏。此前公网首步 8.561 秒及共用 HUD 未就绪的问题仍见[上一轮发布与失败记录](../r16-player-pack-recovery-20261111/production.md)。未通过整游戏验收的本机字体路由候选未发布；整体 1:1 Goal 继续 active。
