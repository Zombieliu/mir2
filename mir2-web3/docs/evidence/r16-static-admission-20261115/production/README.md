# 图片排队候选的两场公网与回退

2026-11-15（Asia/Shanghai）：源码 `05d233ed248e8b78128580ae71e7618f4338ab5c` 曾发布到 `dpl_HSPVqhc6HiBGTWgDBgRohEYrFTkF`。第一场通过原完整门槛，追加的第二场未通过：Native 在首画面后 **189105 ms** 出现，超过原 **120000 ms**，113 项预热也未在验收窗口完成。**已实际回退，不保留该候选为正式版本。**

当前 [numeron.obelisk.build/game](https://numeron.obelisk.build/game) 为此前通过验收的 `20de5ea4fd93b3c61e4aa590dca2ea2d8f5d2339`／`dpl_AWssgszkpKJznMksLsP7hiRFK6U8`。独立 Vercel API、公开版本和 Gateway 健康读回及回退后的简繁扫码均通过。[最终状态](final-readback.json) · [本轮资格为 false](qualification.json) · [此前正式验收](../../r16-font-fallback-chain-20261115/production/README.md)。

## 发布、检查与回退

CI 实现 `94497a6094a3ac29c9366c26f325df19a3b0a19e` 只更新已审阅的来源和回退绑定；83 项发布保护及 30 个前端输入绑定见 [冻结来源](../release-source.json)。阶段任务成功不代替实际公网体验。[四个工作流、写入与流量记录](release-workflows.json)。

| 操作 | 工作流 | 结果 |
| --- | --- | --- |
| stage | [43653655905](https://github.com/Zombieliu/mir2/actions/runs/43653655905) | 创建候选，Vercel 随后 READY；正式仍为 20de |
| check | [43654213483](https://github.com/Zombieliu/mir2/actions/runs/43654213483) | 受保护入口 15 项及扫码待确认通过；零写入 |
| promote | [43654581118](https://github.com/Zombieliu/mir2/actions/runs/43654581118) | 受保护／公开入口各 15 项及扫码待确认通过；切到 HSP 候选 |
| rollback | [43656160859](https://github.com/Zombieliu/mir2/actions/runs/43656160859) | 第二场未通过后，流量实际回到 AWss／20de |

四次工作流均已结束，后端发布任务均跳过。Gateway 始终为 `fd3fcedc8b8a4d0b6bb16f6c45cd57a77364c8b5`，固定素材 `20261108-r16-08ec86d5105c`、运行包 `bevy-0c553a754308e5b8` 和存档规则保持。没有 DNS、主官网、服务器配置、资产发行或存档迁移。

## 两场原始门槛结果都保留

两场均为普通系统网络、独立冷浏览器、官方手机尺寸钱包 PWA、Testnet 虚拟 Passkey、一次实际登录签名确认、一个内部 QA 钱包及新角色。**总计两个内部 QA 钱包／角色，不能作为两位外部玩家。** 没有资金或资产交易；实体手机相机和公开跨图未测。

23 份脚本、92 个缓存体参考和六份地图几何输入在执行前冻结。原判定器 SHA `17086d553a3eebddb9a656d09c94024417081656e1e3c939a6bc3916f61ee402`、8 秒移动和 120 秒 Native 限制保持；180 秒补充观察没有改变原判定。[原判定 R1](original-acceptance-r1.json) · [原判定 R2](original-acceptance-r2.json) · [私人原始文件摘要与脚本绑定](private-artifact-pins.json)。

| 检查 | R1 | R2 |
| --- | --- | --- |
| 原完整判定 | true | **false** |
| 钱包登录确认／建角 | 通过 | 通过 |
| 普通走一步，输入至权威 ACK | 207 ms | 133 ms |
| 点击进入至首画面 | 6854 ms | 6679 ms |
| 首画面后完整运行包 | 12783 ms | 90911 ms |
| 首画面后共用 Native | 58123 ms | **189105 ms，超过 120000 ms** |
| 六套原字体完整 SHA | 通过 | 最终通过 |
| 指定 113 项预热 | 113/113，失败 0 | **验收窗口内未通过；末次 HUD 图为 108/113、5 失败** |
| 共用 HUD／Q／Worker 恢复 | 通过 | 最终通过；不改判超时 |
| 正常注销／返回选角 | 通过 | 通过 |
| 页面异常／console error | 0／0 | 0／10 |
| 资源错误 | 0 | 8 次 504，逐地址保留 |
| 桌面与钱包浏览器 | 均关闭 | 均关闭 |

[R1 分项和计时](public-r1.json) · [R2 分项和计时](public-r2.json)。首画面分别在脚本观测起点后 110022 ms／78665 ms，这个起点包含钱包等操作。8 秒门槛针对移动 ACK，Native 时间从首画面开始；不能把它们写成从打开网页到全部界面只需几秒，也不能从两次不同网络状态推导稳定提速比例。

## 92 个历史失败地址的完整缓存体

旧版曾留下 97 次错误、归并为 92 个地址。此次第二场在原正常注销操作之后，直接读取当前 schema／namespace 的 CacheStorage：**92 个完整正文全部匹配参考字节数和 SHA，缺失 0、错体 0**。这次读取没有发送请求、预热、修复或注入缓存。[完整逐体结果](final-cache-bodies-r2.json)。

这证明这 92 个地址在第二场最终完整缓存，不证明它们都在原时间窗口前就绪；也不证明所有其他图片或新出现的 8 个错误地址都已恢复。R2 的 113 预热及 Native 原门槛仍失败。

第一场新增的缓存观察器有错误：按 `mir2-asset-cache-static-` 前缀筛选，但实际格式为 `mir2-asset-cache-sw8-static-`，选到零个缓存。原结果的“92 缺失”无效，不能当成产品故障。最初合成夹具也沿用了同一错误，未覆盖真实 Worker 格式。原数据和摘要保留，未追写为成功。[错误及更正](observation-correction.json)。

更正后，测试直接执行实际 Worker 的 `cacheName` 生成名称，区分“没有当前缓存”与“当前缓存中没有该条目”；旧 schema、旧 namespace、截断、错体、过大正文、认证 URL 以及观察超时均有覆盖。15 项检查和真实 Chrome 通过；真实 Chrome 的检查期间网络请求为 0。[检查输出](observer-checks.txt) · [浏览器回执](observer-browser-r2.json)。随后才执行第二场普通公网流程，未重跑或改写第一场。

七条许可资源来源都显式纳入新观察器。R1 实际出现三个上游来源前缀；R2 实际观察到全部七个。两场瀑布分别 1062／1418 行，均零丢弃、未截断。许可覆盖不表示每条来源都成功，旧版本上的字体 404 等也保留，不能用当前来源成功掩盖。

## 下一步依据

第二场应用源运行包收到 200 响应头，随后正文停滞，触发既有空闲恢复；CDN 后备完整传输又耗时约 70 秒。TC 完整字体约传输 101 秒。两场 Page-CDP 都记录到：SC 第一次页面请求在运行包完整正文之后约 0.2 秒发出；第二场随后又等待约 88 秒。[运行包与字体记录](r2-tail-diagnostics.json) · [区分页面与 Worker 的字体顺序](font-order-observation.json)。

源码有意把字体预热放在最终运行包成功之后，以减少正文竞争。下一轮先做受控对照，检查提前预热一套大字体与运行包的竞争和串行等待，继续要求相同完整字体、正文、113 项和原 8／120 秒，不盲目移动预热或上调门槛。停止原样重新发布此候选。

## 回退复验与截图

回退后简中、繁中各一次实际二维码、同意图待确认、正常取消、取消后零轮询及浏览器关闭通过；这两次没有再作钱包签名。[简繁回执](rollback-qr.json)。

四张原图均已审阅：

| 场次 | 共用任务窗 | Worker 重启后 HUD |
| --- | --- | --- |
| R1 | [图](r1-quest-shared-zh-TW.png) | [图](r1-public-hud-after-worker-restart.png) |
| R2 | [图](r2-quest-shared-zh-TW.png) | [图](r2-public-hud-after-worker-restart.png) |

中途缓存失败／恢复状态和后续完整界面均按原图保留。截图仍有英文、原始地图标识与其他未对齐处，字体及 Native ready 不代替语言／像素验收。其他目标、地图、公开跨图和双钱包占位、断网恢复、三职业完整 1–30、技能、阿洛、跨端同档、实体手机及整体 100% Candidate／人类 Accepted 继续开放。29 份无关内容 WIP 的 SHA 保持，私人账户、认证流量及浏览器档案未入仓库。
