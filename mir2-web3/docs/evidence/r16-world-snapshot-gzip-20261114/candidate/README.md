# 独立网页候选与上线前检查

冻结源码 `9244ce3dd7cd2129a70e650be1430e971b6c96ab` 已完成 Web 云端构建。
[独立 API 读回](ready.json)确认正确游戏项目、R16 分支、源码与 READY；
候选地址受 Vercel 保护，不是正式游戏入口。

Stage [43593027496](https://github.com/Zombieliu/mir2/actions/runs/43593027496)
使用发布工具提交 `b0cc37a3db`，仅创建一个游戏候选，前后正式 target 与
`numeron.obelisk.build` 的 alias 均保持 `dpl_Ae3hNyFygQuSMZDesBXCZDpUECTY`。
没有切换官网、DNS 或网关。完整冻结来源与本地游戏证据见[上级记录](../README.md)。

新增 `numeron-qr-check`，复用既有候选身份、源码闭包、固定素材、15 项完整字节
与 SHA、受保护游戏页面和真实二维码 create/pending 检查；执行前后复核原正式映射。
它不执行部署创建、promote、rollback 或域名写入。扫码检查会创建一个短期登录
请求，不执行钱包确认、交易或进入游戏。受保护候选检查与普通公网完整试玩分别记录。

本机三组发布保护共 80 项通过，包含损坏资源、错误源码/READY、检查期间正式映射
被其他操作改变，以及无效二维码正文的拒绝行为。原有发布与回退检查保持通过。
云端 [43594322119](https://github.com/Zombieliu/mir2/actions/runs/43594322119)
已实际通过全部 15 项字节及 SHA、真实 create/pending 和前后正式映射检查。
[检查与原始输入 SHA](checks.json)记录工具源码、完整素材及执行范围；没有 promote。

SSH 已经验证 `ubuntu` 身份可用，只读确认在线 TEST 为 `fd3fcedc8b8a` / ELF
`11fc151644ecb75ccb5a86789cefb2010a062ef48c3a9ddfd5ccf29b67dfe926`。
该服务仍在其他容量诊断中，本轮保持在线服务、配置和存档不动。
既有公网 8 秒移动确认、120 秒 Native 冷启与完整 1:1 验收继续开放。

## 当前 E6 正式站的完整被动对照

一次普通 Chrome `/game`、官方手机尺寸钱包 Testnet 登录确认、正常建角、进比奇、
一步键盘移动、繁中共享 Q、六字体完整 SHA、113/113 预取、Worker 停启和正常注销
最终通过，两端浏览器关闭。移动发包到权威回包 136ms；不把手机尺寸浏览器模拟
写成实体手机相机验收。[逐项结果](current-e6-control.json) ·
[共享任务窗当时截图](current-e6-shared-quest.png)。截图发生在后续预取复核之前，
左下仍显示 51/113 与 5 项失败；后来恢复到 113/113 的结果单独见回执，不倒填截图。
NPC 的 Jane/Scott 等原名和英文开发告示仍可见，不能由字体及 Q 可用宣布语言或像素 1:1。

**完整冷启动仍失败。**运行包正文在首帧后 146582ms 收完，共享 HUD 最后在首帧后
211585ms 观测就绪，均超过原 120000ms；后续 180 秒补充观察不改变原门槛。
功能脚本的 `passed:true` 仅指它的功能检查，综合冷启动资格明确为 false。
0 条 page error、132 条 console error 原数保留，没有据最终恢复宣称无错误。
这次是 E6 / 在线 fd3 网关的对照，不能代替 9244 候选的公网验收。

[完整静态时间线](current-e6-static-waterfall.json)记录 1423 条允许的静态 GET，
包含 591 条 Worker 上游事件，未截断，结束时尚有 21 条请求未完成。
其中几张 `ChrSel/0..18` 在开门阶段发出，到进城后仍持续下载，最长观测 175435ms；
与运行包、字体及元数据请求重叠。这能证明重叠和延迟，不能单独证明取消后的收益。
接下来对开门预载的取消、截止与并发做受控修复和对照，保持素材与原 8/120 秒门槛。

复测后独立 [API 与 Gateway health](post-control-production.json) 仍为 E6/Ae3/READY，
在线 fd3，六项在途计数均 0。本轮没有更新网关或正式域名。

## 静态观测工具的边界

新增 `scripts/lib/r16-static-waterfall.mjs`，只观察一个自有浏览器 context 的允许静态
路径和固定 CDN 前缀。工具不读正文、header、cookie 或认证载荷，丢弃 query/hash，
排除钱包、身份、支付和 WS 请求；区分页面与 Worker，不把重复层的字节相加成流量。
最多 4096 行，溢出、未完成请求与未完成 sizes 读取均显式记录。

Playwright 1.55.1 必须在启动浏览器前设置 `PW_EXPERIMENTAL_SERVICE_WORKER_NETWORK_EVENTS=1`
才能收到 Worker 上游事件；回执分别记录请求此能力与实际观测行数，不能只凭 flag
宣布 Worker 覆盖。8 项单元检查及真实 Edge 本地夹具均通过；夹具观察 6 条请求、2 条
Worker 事件，包含慢正文、503 和网络失败并验证隐私过滤，随后关闭浏览器和服务器。
首轮 URL 规范化反例失败已修复，原失败日志 SHA 保留在[检查记录](checks.json)。
