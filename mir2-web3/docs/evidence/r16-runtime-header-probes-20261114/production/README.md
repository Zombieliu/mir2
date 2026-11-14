# 公网功能通过，冷启动失败并回退

`5b011e5d2f` 已尝试发布。普通公网浏览器的完整功能回放通过，但原冷启动门槛 **未通过**，
已恢复 E6，并完成简繁扫码、待确认与取消复验。代码及本地资格保留，不原样重发。

| 步骤 | 回执 | 结果 |
| --- | --- | --- |
| 创建独立候选 | [43608225138](https://github.com/Zombieliu/mir2/actions/runs/43608225138) | 成功，正式未变 |
| 首次保护检查 | [43608781498](https://github.com/Zombieliu/mir2/actions/runs/43608781498) | false，无写入；候选尚未 READY |
| READY 后保护检查 | [43609234067](https://github.com/Zombieliu/mir2/actions/runs/43609234067) | 15 项完整字节 / SHA 与扫码 pending 通过，无写入 |
| 发布候选 | [43609665021](https://github.com/Zombieliu/mir2/actions/runs/43609665021) | 成功，前后各 15 项资源及公网扫码分项通过 |
| 原条件分类 | [完整结果](public-acceptance.json) | 功能 true，冷启动 false，整体 accepted=false |
| 恢复 E6 | [43610813735](https://github.com/Zombieliu/mir2/actions/runs/43610813735) | 成功，独立 API / `/version` 一致，简繁出码和取消通过 |

首个检查在 `15:13:03.101Z` 被拒绝，Vercel 到 `15:13:23.154Z` 才记录 READY；保护错误码
同时覆盖身份 / 根目录 / 未就绪，因此没有据此修改或绕过来源保护。READY 后同一候选通过
原检查。[构建时间线](candidate-ready.json) · [五轮发布操作](release-workflows.json)。

当前正式为 `e6c923b7c669cb061a7a03703f05fadd91211c97` / `dpl_Ae3hNyFygQuSMZDesBXCZDpUECTY`，
项目及域名保持原值，网关仍是 fd3。本轮没有部署或重启后台。
[独立映射](final-production-mapping.json) · [公网版本 / 健康](final-public-service-status.json)。

## 功能回放与实际时限

普通 Chrome 从无参数 `/game` 出码，在官方 Testnet 钱包的手机尺寸浏览器中只批准一次登录。
新建专用测试角色进比奇，普通键盘行走的服务器确认 **3810ms**，通过原 8 秒。
最终六套完整字体 SHA、113/113 预取、共享 Native HUD / Q、Worker 停启和普通注销保存通过，
回到选角，桌面与手机侧自有浏览器均关闭。没有充值、交易或实体手机摄像头验收。

运行包在首帧后 **154494ms** 完成，共享 Native HUD 的观测为 **247642ms**；两项均超过原
120 秒。原验收分类器在发布前冻结，7 项检查包括拒绝旧 E6 的“功能通过、冷启失败”实例，
不把额外观察时间当作延长原门槛。[原分类](public-acceptance.json) · [分类器](acceptance-classifier-contract.json) · [分项与时间](public-result.json)。

公网确实执行了这次新增的恢复：应用源与 CDN 各 8 秒无响应头后，第三次回到应用源并完成。
第三次请求等待响应头约 **95204ms**，正文约 **43186ms**。这说明恢复可用，尚未解决长等待；
不能从这一条记录认定是浏览器竞争、代理、源站或压缩的根因。

完整静态白名单观测 **1371 行 / Worker 537 行 / 末尾 7 项未完成 / 无截断**。
开门仅两个 Page 所有的 0–18 号帧请求，角色选择 20–35 号帧及上游重复行不算开门帧。
Page 和 Worker 不能相加作为实际网络字节。[原静态瀑布](public-static-waterfall.json)。

最终字体检查为 6/6；普通浏览器记录 128 条 console error、0 条 pageerror。启动观察另记录
119 条 error / 1 条 warning 和 124 条资源错误，观察跨度不同、存在重叠，不相加或隐藏。
[字体原字节](public-original-fonts.json) · [错误观察](browser-error-observations.json)。

[共享 Q 实画面](public-shared-quest.png)捕获时显示 71/113、失败 2 项；后续恢复到 113/113
另见最终回执，不将该截图标为最终预取完成。[Worker 停启后的画面](public-hud-after-worker-restart.png)
也已审阅。中文 / 英文混杂、raw NPC 名、文字像素与其余全局 1:1 条件继续开放。

## 恢复后检查与下一项

恢复后简体、繁体各一个全新普通 Chrome 上下文，从无参数 `/game` 真实点击一次出码，
二维码矩阵、官方钱包地址及同一 intent 的 pending 通过；普通取消返回登录，之后零轮询，
两浏览器均关闭。没有再次批准登录或创建角色。
[简体](qr-zh-cn-receipt.json) · [繁体](qr-zh-tw-receipt.json) · [汇总 / 原件 SHA](checks.json)。

## 隔离同包读取

恢复 E6 后，另开全新普通系统网络 Chrome，在 `/version` 空页串行读取同一个固定 WASM。
该页不登录、不运行场景，Worker 被禁用；无代理 / DNS / TLS 覆盖或请求拦截。三次 200、
31452436 解码字节及 SHA 均一致，自有浏览器关闭，无钱包、Relay、Gateway 或 WS 动作。

| 路径 | 响应头 | 总耗时（含正文和 SHA） | 缓存观察 |
| --- | ---: | ---: | --- |
| 应用源 | 856.6ms | 51326.8ms | br，Vercel HIT，age 52056 |
| 当前 CDN | 834.4ms | 54146.5ms | gzip，CF HIT，age 138980 |
| 同 CDN 的 hotlink-ok 路径 | 166.9ms | 52899.2ms | gzip，CF HIT，age 139034 |

[原读取回执](isolated-runtime-read.json) · [输入 / 脚本 SHA 与边界](isolated-read-inputs.json)。
这些是恢复 E6 后的 warm / 未配对缓存状态，原整页为 5b011，且没有场景竞争；不能将三次
读回当作冷启动或因果 A/B，也不证明游戏加载排程一定是根因。它确认该时段三个路径可读，
无需通过修改文件、字体或超时来掩盖 95 秒长头。

下一项在同构建、同场景的对照中补齐安全缓存 / 请求优先级观察，再验证关键运行包与预取
的排程；需要时另核对代理 / 源站。原 8 / 120 秒、六字体、113 项和回退保护保持；
不原样重新发布 5b011。
