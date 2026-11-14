# 分源候选：公网失败、已回退

2026-11-14，当前正式站仍是
`e6c923b7c669cb061a7a03703f05fadd91211c97`／
`dpl_Ae3hNyFygQuSMZDesBXCZDpUECTY`。
回退后新 Chrome 的简／繁真实出码、二维码像素匹配及 pending 均通过，
之后独立 API 于 07:10:09 UTC 再确认 READY、正确项目和 R16 分支。
[最终映射](final-production-mapping-after-qr.json) · [回退后扫码](rollback-qr-browser.json)。
本轮没有在回退后重新做一次完整进城或长路线；不把扫码通过当成这些范围通过。

## 发布与失败事实

源 `7140aae96c41986968aadf040436e0ab396e1fdb` 的
[282项检查与四会话](../README.md)先通过，再进行受保护发布。

| 阶段 | 结果 | 回执 |
| --- | --- | --- |
| Stage `43567414882` | helper `de7129b61d`；创建 `dpl_iYGHW7kJ219kFfwWYjdSFeiek4xu`，正式映射保持 Ae3；独立 API 核对 READY／源码／分支／项目 | [Stage](stage.json) |
| Promote `43568139830` | 仅一次 game promote，无补偿回退；15项 protected／public完整字节与SHA一致，包含44059字节新Worker；首次QR pending通过；06:52:33 UTC独立API确认新正式映射 | [Promote](promotion.json) |
| 新普通浏览器全流程 | 真实出码、官方手机尺寸钱包Testnet登录确认、建一名QA角色、进比奇、最终接受一步到(289,616)、繁中备用任务窗、113/113和正常退出通过；整轮false | [失败回执](failed-signed-browser.json) |
| Rollback `43569019844` | 恢复Ae3，工作流及独立API核对通过；新简／繁出码pending、关闭浏览器通过；之后再确认正式映射 | [回退](rollback-sequence.json) |

公网首次一步从 (288,616) 向右，103404ms 发包、116690ms 收到正确
`UserLocation`：13.286秒，超过原8秒。CDP原生WebSocket帧间隔13.283秒，
同窗KeepAlive RTT11.137秒，其他样本约134–171ms。这里不是仅仅页面晚画出位置；
仍需同步服务器／Zone／代理链路记录才能区分原因。观测到的 Gateway health
revision 为 `8f3099f4a0114a7825ff61db36b548d3bfb5b3fa`，不把它与Web源码混写。
本次输入是普通一步键盘，不是公网长自动路线；若自动路线遇到同样超过8秒的确认，
按现有规则会停止，不能靠延长客户端门槛冒充修复。

此外，原120秒资格false，其后的180秒附加观察也未见共享HUD就绪，缓存只完整收到4/6
字体：TC、Latin、Devanagari、Thai。SC和Arabic在快照时缺失，整套字体资格false。
Arabic上游有完整结束记录，也不倒填为该次缓存检查通过。
[字体快照](failed-font-cache.json) · [真实Worker上游](font-upstream-observation.json)。

运行包正文到首帧后153.493秒才收完，原120秒资格仍false：先应用源8秒无头，
CDN得到200后无正文15秒中止；CDN重试约44.55秒才有头，短暂正文后再停顿，
随后应用源恢复请求才收完31,452,436字节。恢复机制起效不能代替原冷加载门槛。
原生共享Q及Worker重启资格未获得。保留3条console、0条page、1条资源HTTP错误，
以及144条 `net::ERR_ABORTED`。
[字体／运行包和WS传输记录](runtime-font-transport.json)。该冻结收集器只记录
字体、WASM和运行包glue，**没有完整场景资源瀑布**，不能由此排除其他资源竞争。

因为必需浏览器门槛失败，没有生成 `signedBrowserPassed:true`，后续完整静态
审计没有执行。切流前后15项字节检查的通过仍单独保留，不代替游戏功能。
桌面与钱包浏览器均已完整关闭；官方钱包只进行一次受约束登录确认，无禁止操作。
实际手机相机／实体Windows与全局1:1继续开放。

## 下一步

保留[本地候选](../README.md)及八组静态隔离的全部通过和失败，不再原样重发。
先分别补齐首步确认的服务器队列／传输关联，以及不包含认证载荷的完整静态资源
被动瀑布，解释独立读取与整页启动为何不同；再选择运行包恢复与资源调度的窄改动。
原8秒／15秒／120秒、完整六字体和权威到达条件保持不变。Goal仍active。
