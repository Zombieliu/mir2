# 主字体候选公网失败与回退

2026-11-15，固定前端源 `5d7bf8c2ea58d2ac68bbca5f18c8240fdc929d45`，候选 `dpl_BoNqc3ESVREqjmXEJtMT2VAtLTFw`。Stage、Check、Promote 均通过，保护入口和公开入口各 15 项；完整公网第一场 **未通过**，未运行第二场，未放宽原 8 秒移动、120 秒 Native、六完整字体／113 项及正常注销门槛。

[原判定](original-acceptance-r1.json) 与 [分项回执](public-r1.json) 保留：扫码签名进城、首步 135 ms、六套完整字体、113/113、共用 Q、Worker 恢复通过；首帧后运行包正文 25.694 秒，Native 101.946 秒。内部 QA 使用官方钱包 Testnet 流程，不能代替实体手机或外部真人验收。

首先失败的是新手怪物行走包：原 JSON 首次响应 200，但正文只观察到 376,831/764,724 B 后取消；第二次 20 秒内未完整到齐，编译恢复包在最初画面资格检查时尚未开始。后续编译块响应 200 不等于最初期限内已经解析、发布并画出行走帧，因此保留 `PUBLIC_VISUAL_PACK_BODY_NOT_OBSERVED:entry-monster-walking`，不以其他完整缓存或字体通过覆盖。

另有任务窗口问题：[按键与窗口观察](quest-owner-observations.json) 显示，日记 ready/open 为 true，而主 HUD ready 为 false；第二次 Q 被 `quest-modal` 拦截。最终正常退出没有发出 Logout，保留 `LOGOUT_COMMAND_NOT_OBSERVED`。两项分开调查，不认定字体调度已经证明是窗口问题的原因。画面：[任务窗失败](quest-failure.png)、[Worker 恢复后 HUD](hud-after-worker-restart.png)。

回退工作流 [43665380646](https://github.com/Zombieliu/mir2/actions/runs/43665380646) 成功，仅执行游戏项目 rollback；[回退核对](rollback.json) 确认 target/alias 均回到 `dpl_AWssgszkpKJznMksLsP7hiRFK6U8`。独立 Vercel 查询确认同一游戏项目、READY/production、源 `20de5ea4fd93b3c61e4aa590dca2ea2d8f5d2339`；域名 `/version` 也为该源。Gateway health 200/ok，仍为 `fd3fcedc8b8a4d0b6bb16f6c45cd57a77364c8b5`，未发布后端。

[回退后简繁扫码](rollback-qr.json) 均通过真实二维码矩阵、同一意图 pending、正常取消、取消后零轮询及浏览器关闭。此补验没有钱包确认，不冒充另一场完整游戏回放。

本地候选、寻路反馈与主字体源码仍保留在 R16 分支；公网没有保留这次候选。下一轮先修任务窗口自身的关闭权限，并补查完整行走包恢复，不能原样重发。100% Candidate、其他地图／任务、三职业 1–30、阿洛和最终人类 Accepted 均未关闭。账号、登录轨迹及原始存档留在本机，发布文件仅含必要事实，[原始文件校验值](private-receipt-pins.json) 可用于复查。
