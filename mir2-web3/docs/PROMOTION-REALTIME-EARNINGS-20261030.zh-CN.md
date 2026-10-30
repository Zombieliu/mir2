# 团长累计佣金实时显示｜2026-10-30

## 范围

用户确认已有邀请、充值与返利记录，本轮不再创建短时测试赛季，也不提前结束活动。目标是团长在活动期间看到累计佣金变化，活动结束后核账结算、付款。

本轮仅更新原游戏／推广 Vercel 项目的前端。账号、角色、邀请、充值、佣金与历史付款数据不迁移；不改活动开关、不转账、不启动 Docker、不发布 Windows 客户端。宣传主站项目保持原部署。

## 页面行为

- 登录后的总览、成员和收益页每 10 秒读取一次当前选中赛季；页面重新可见或获得焦点时可立即刷新。
- 后台标签页、正在操作或钱包签名窗口开启时暂停自动读取；慢请求与连续焦点事件不会形成并发请求队列。
- 手动刷新同时刷新当前成员分页或结算记录。切页、退出登录与后续请求会使旧请求失效，迟到响应不能覆盖新状态。
- 活动期间显示“本赛季累计佣金（待结算）”；已封账活动显示“本赛季已封账佣金”。收益页显示赛季结束时间和到秒的最近数据更新时间。
- 累计预估佣金不依赖付款开关。计佣和实际钱包付款是两个阶段；本轮保留 `payoutEnabled=false`。
- 十种现有语言同步补充上述文案；微额收入保留六位小数精度。

## 数据边界

默认七天测试活动为 `numeron-playtest-20261029`。本轮只读检查时有 4 位团长、3 位成员，尚无合格月卡收入，所以该活动累计佣金为零。

已有真实链路验收记录属于 `numeron-chain-acceptance-20261029-fullchain`，该活动已经封账。10 USD 月卡仅有 15235 毫秒服务时间与当时短活动重叠，合格收入为 58 microUSD、佣金为 5 microUSD，历史已确认付款为 0.000005 测试 USDC。不会将旧活动收入重复计入新赛季。

历史入口：<https://mir2.obelisk.build/promotions?campaignId=numeron-chain-acceptance-20261029-fullchain&lang=zh-CN#earnings>。

推广测试游戏使用 `/promotions/play` 和独立 `numeron-promotion-testnet` realm。原 Windows 测试服数据不自动进入该 realm，不能把普通测试服充值当作推广活动入账证据。

## 验证

- 34 项刷新控制、金额／文案、赛季选择和相邻计费／游戏代理回归测试通过。
- 完整前端 TypeScript 检查通过。
- 10 项隔离浏览器场景通过；实际等待 10.032 秒验证无点击金额更新，另验证同页结算／成员刷新、隐藏暂停、切页与退出迟到响应，以及身份切换后重置成员分页。桌面 1440 和手机 390 像素均无横向溢出、无控制台错误。
- 额外使用 UTC 服务端配合 Asia/Shanghai 浏览器，验证 SSR 时区占位与客户端初始化后本地时区显示一致，不出现 hydration 错误。
- 浏览器与发布证据在 [promotion-realtime-20261030](evidence/promotion-realtime-20261030/)。浏览器业务数据使用显式本地 fixture，与线上只读检查、历史链上验收分别标注，不能当作新充值或新转账的凭据。

## 发布

使用原 CI 中已有的 Vercel 凭据，从同一不可变提交创建不分配正式域名的候选。候选通过内容、静态资源、活动开关及 `/game`、宣传主站回归后才提升；失败仅回滚游戏／推广项目。

首轮候选检查通过，但提升后公网仍返回旧推广页面资源，已自动恢复原部署。发布工具随后补上 Vercel 异步域名分配完成检查（`lastAliasRequest`），同时确认主域名及 Cloudflare Worker 使用的稳定源站域名均指向候选，再等待普通 `/version` 和页面内容通过。此流程参考 [Vercel CLI 56.4.1 的 promotion 状态实现](https://github.com/vercel/vercel/blob/vercel%4056.4.1/packages/cli/src/commands/promote/status.ts)。

Cloudflare 只读检查确认主域名由现有 `mir2-web3-domain-proxy` 转发至 `https://mir2-web3-web.vercel.app`；未修改 Worker、路由、缓存或 DNS。

最终修复版已于 2026-10-30 19:15 UTC+8 完成上线及普通公网 HTTP 验证：前端与发布工具源提交 `0a05d9e1fcd61a9427c40e18e5fcf177960a7bd5`，部署 `dpl_2yXeENZ4qrg69FhcThRW7o7H9KkB`。[成功发布记录](https://github.com/Zombieliu/mir2/actions/runs/41046532226)确认计佣开启、付款关闭及宣传站部署不变。

实际日志证实生产指针先更新，两个别名仍指向旧部署；等待异步任务成功及两个别名切换后，普通公网页面、13 个推广资源和版本检查全部通过。详细边界及发布证据见 [VERIFICATION.json](evidence/promotion-realtime-20261030/VERIFICATION.json)。本轮未新增真实充值或转账，新的页面仍待用户目视验收。

随后公网浏览器检查发现原有页脚直接计算本地时区，会在服务器 UTC、浏览器 Asia/Shanghai 时产生 React hydration 文本差异。UTC 浏览器无错误、Asia/Shanghai 浏览器可稳定复现的只读对照证据保留在 [HYDRATION-TIMEZONE-DIAGNOSIS.json](evidence/promotion-realtime-20261030/HYDRATION-TIMEZONE-DIAGNOSIS.json)。前端改为先渲染中性占位、客户端初始化后显示浏览器时区，已在 UTC 本地服务器通过上述 10 项检查。

最终公网浏览器复验于 19:16 UTC+8 全部通过：版本精确匹配最终提交；UTC 和 Asia/Shanghai 均无 hydration／控制台错误；匿名页面观察 11.383 秒无新增自动轮询；桌面和手机无横向溢出；主站页头、页脚、手机导航及 `/game` 渲染正常。全过程未登录、未注入 fixture、未发送 POST 或其他业务写入。最终报告为 [PUBLIC-BROWSER.json](evidence/promotion-realtime-20261030/PUBLIC-BROWSER.json)，初次失败报告另行保留，未覆盖。
