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
云端执行结果将在独立回执中补充；不能由单元测试代替实际候选检查。

SSH 已经验证 `ubuntu` 身份可用，只读确认在线 TEST 为 `fd3fcedc8b8a` / ELF
`11fc151644ecb75ccb5a86789cefb2010a062ef48c3a9ddfd5ccf29b67dfe926`。
该服务仍在其他容量诊断中，本轮保持在线服务、配置和存档不动。
既有公网 8 秒移动确认、120 秒 Native 冷启与完整 1:1 验收继续开放。
