# 字体分段正文进展与失败来源切换候选

基于 Windows R16 `codex/windows-player-journey` 的
`2b7b5d7079300f45315e172d4c58784aaed6fb5c`，本地候选 `r16-font-body-7d3c6c83`。
本轮修复、215 项检查、真实 Chrome 及隔离构建通过；英文完整流程和繁中补充流程通过。
**尚未发布**。正式仍为 e0／Fo；新的同源 CI、旧档隔离和两场原公网门槛仍须完成。
整体 1:1 Candidate、Accepted 均未完成，Goal active。

## 依据与实现

上一轮 306 试发第二场 Native 为首帧后 **128,080 ms**，超过原 120 秒，已成对回退。
原时间线中，1 MiB 范围正文有进展仍在响应头后 15,002 ms 被取消；两个来源超时后，
还会在更小范围及整包上重复等待。最后完整 SC 到达后 394 ms 才接管共享 HUD。
[原失败时间线](preceding-public-failure.json)和[原判定／回退](../r16-cjk-progress-20261116/production/README.md)保留。

- 网络分段改为 **15 秒没有非空正文才取消**，不再把持续下载的 16 秒正文当停传。
  空 chunk 不续期；完整长度、EOF、各片 SHA 和整字体 SHA 均须通过。
- 缓存正文仍采用固定 15 秒验证期限，缓存操作仍为 750 ms。范围传输的 24 秒已校验
  无进展期限、48 秒绝对上限、8 秒响应头期限和全局两个下载名额保持。
- 仅在当前字体的一次配置快照／逻辑请求中记住范围传输失败的 URL；避免对已失败来源
  反复尝试小范围。范围失败可让独立 app 整包优先，即便已经保留部分校验片。
- 范围失败不永久禁用来源，也不排除整包镜像。app 整包损坏后仍可用完整镜像；
  新请求可重试恢复的来源。主动取消及外层期限不误记为来源故障。

SC 63 片、TC 46 片与全部六套完整原字体保持，113 项 HUD 资源不减少；原公网
移动 8 秒、自然帧 65 秒、Native 120 秒和扫码两次各 20 秒不增加。
Worker 为 73,875 字节，SHA `2333c5f6c64c05c2485c65bf4965df859e25d0bc446f72ac72ab716885f4aba2`。

## 验证与保留失败

**215 项通过，0 失败／忽略**：152 项 Worker、续传、来源、取消及生命周期回归，
63 项相邻准入、运行包预取、扫码准备。8 项新增测试已包含，没有重复计数。
旧 Worker 的五个目标测试中，四个失败，固定缓存期限一个通过。
[152 项](focused-final.log) · [63 项](adjacent-final.log) · [旧代码预期失败](before-regressions.log)

真实 Chrome 读取实际 HTTP 与 CacheStorage，无预填字体：

| 场景 | 原值／结果 |
| --- | --- |
| SC 首个 1 MiB 分四块每 4 秒到达 | 16,118 ms，63 片完整，无整包重下，通过 |
| TC 相同场景 | 16,099 ms，46 片完整，通过 |
| 旧 Worker 相同 SC 场景 | 24,241 ms，59／63 片，原断言失败，保留 |
| 两个当前来源不发响应头 | 16,091 ms，四次已准入范围后一次 app 整包，通过 |

三个通过场景均在重启后核对六字体完整 SHA、FontFace 解码与绘制。
[六字体画面](six-original-fonts.png)及 `chrome-*.json` 保存实际请求和关闭状态。

首次响应头故障用例实际完成恢复，但原服务器观察器把取消中的无响应头请求寿命也计为正文，
原 `peak<=2` 观察失败，记录保留在 [原观察器回执](chrome-header-failover.json)。
更正后的观察器同时保留原始请求峰值 **4**，并统计真正发出 200／206 头后的正文峰值 **1**；
只更正该故障场景的测量范围，原非故障场景的请求峰值检查、两个下载名额及应用均未改变。
[更正后回执](chrome-header-failover-body-observer.json)。

严格 Next／TypeScript 构建通过，43,889 ms，BUILD_ID `NZunMJ6fCFPI4XoqB3fyG`；
对照 2,181 个已提交 app/lib 文件，保留一个固定生成输入，运行包保持。

| 本地完整流程 | 首步 ACK | Native 首帧后 | 结果 |
| --- | ---: | ---: | --- |
| 英文默认后端 | 123 ms | 17,792 ms | 完整通过 |
| 繁中 WebGL2 原流程 | 122 ms | 20,676 ms | 六字体通过；正向旧缓存清理断言失败，未完成后续存档检查 |
| 繁中 WebGL2 补充流程 | 114 ms | 24,693 ms | 明确注入一份过期缓存后完整通过 |

原繁中流程观测删除数为零，没有记录测试启动前是否存在过期缓存；不能将它改判为通过，
也不能据此认定产品未清理。补充流程明确验证旧缓存存在、配置后消失，保留原正向清理断言。
补充流程继承此前记录的原始 CDP 附属地形截图方式，所有游戏、资源、移动、65／120 秒、
Q 和保存断言不变。第一次补验复制私有档案时 D 盘不足，尚未启动浏览器或游戏服务；
[原预检失败](disk-preflight-failure.json)保留，改用 F 盘输出。

两场完整通过的流程都验证了 113 项、六字体、首次／重进 Native Q、Q 零世界写入、
普通 Alt+X 注销、同角色落点重登和保存、原角色及存档不变。
它们复用自有 CacheStorage、清理 HTTP／登录态，**不是公网冷启动验收**。
[英文 Q](english-native-quest.png) · [繁中补充 Q](traditional-native-quest-supplement.png)

15 个私有构建覆盖均已恢复；29 份原内容 WIP 原始 SHA 保持。自有浏览器、代理、Next、
Gateway 均关闭。只迁移了本轮已关闭的测试浏览器档案，保留文件数／长度核验和原回执。
认证信息与账号数据不发布。[验证汇总](validation.json) · [证据／私有回执 SHA](evidence-manifest.json)

## 后续范围

新候选须取得新的不可变源码与发布控制配置、Linux／PG／SHA 和旧档隔离资格，再按原门槛
做两场公网。不得重发已回退的 306、重跑其已完成的控制流程或放宽时限。

应用源码已提交并推送为 `8bf3dbcbeae86c0f4782e2c7dcdd8dd203756efe`。
新发布配置以上版 e0／Fo 为实际回退基准；独立 Vercel alias／部署、Web 版本和网关健康一致。
独立 GitHub 比较核对 6 个提交、106 个文件，两个检查点为 90／23 个文件；
只变更许可的源码／字节 pin 与精确测试，**88 项发布保护通过**，未切生产。
[配置](release/release-pin.json) · [源码闭包](release/source-closure.json) ·
[独立正式读回](release/baseline-readback.json) · [88 项输出](release/release-guards.log)。
新的 [Linux／PG 构建](https://github.com/Zombieliu/mir2/actions/runs/43795056603)仍在进行，
尚未据此声明同源服务包、旧档或生产资格。

首次冷 Q 的备用布局／黑帧、系统英文欢迎、更多地图／断网、三职业完整 1–30、阿洛、
技能及 Windows 实际窗口、实体手机仍按原范围继续。

复测（输出目录需不存在）：

```text
node --test mir2-web3/apps/web/scripts/test-asset-worker-cjk-progress.mjs
node mir2-web3/apps/web/scripts/smoke-asset-font-ranges.mjs --output <new-output> --font sc --failure-mode chunk-progress --chrome <Chrome-executable> --playwright-package <package.json-containing-playwright>
node mir2-web3/apps/web/scripts/smoke-asset-font-ranges.mjs --output <another-output> --font sc --failure-mode header-failover --chrome <Chrome-executable> --playwright-package <package.json-containing-playwright>
```
