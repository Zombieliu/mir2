# R16 选角预下载运行包候选（2026-11-16）

运行包的完整下载与流式编译已提前到普通登录后的选角界面。进入地图后复用同一个不可变包的在途请求或编译结果；原有地图首帧、canvas、唯一初始化和失败隔离继续负责启动 App。当前仅为本地候选，**本轮没有发布生产**。

源码基于 `71a7bf898d1855c4c4a45bbb0a80b51e2a8f2065`，候选 `r16-selection-aef8e659`，构建 ID `ZMCK5Xwe_LW1wLAwN66iZ`。源码、完整性及校验分别见 [source.json](source.json)、[build.json](build.json)、[tests.json](tests.json)。

## 行为边界

- 只有普通选角路径启动预下载；登录、扫码和直接进场保持原先的下载入口。触屏兼容、显式禁用、无 GPU、无有效 manifest、已有/不确定的初始化均不额外预下载。
- 选角到进城共用下载所有者。返回登录、卸载或游戏初始化所有者退役会终止未完成体；包/后端改变则丢弃旧准备，不给错误包接管权限。已失败的编译会传给游戏错误边界，不偷偷再下载一份。
- 预下载不导入 glue、不实例化运行包、不创建 App，也不启动游戏字体/HUD 预热。主游戏仍在首帧后接管，并延用原字体、HUD、重进和正常存档生命周期。
- 抽出的下载/编译核心逐字相同，SHA `765503b4d410b231d440efafd1f7b9e1b905231a8125e85610e8bebeac74a437`。8 秒探测、正文闲置/总时限、字节上限、源顺序、有限恢复与终止条件不变。

## 实际验证

147 项定向/相邻检查通过，失败/跳过均为零。新增用例已接入常用 `npm run test:bevy-runtime-startup`，该108项子集实际通过（已包含在147项中，不相加）；后续仅补此测试入口，应用/依赖输入不变。Chrome 6 场真实 loopback WASM 传输覆盖在途接管、完成后接管、取消选角、取消游戏、损坏体和切换包，浏览器与服务器均关闭。该夹具不是完整游戏性能证据，见 [chrome.json](chrome.json)。

严格 Next/TypeScript 构建通过，44251ms；2179 个既有 app/lib 文件加 1 个新增模块核对，1 份既有生成输入按原锁定值保留，32 份固定输入逐项校验；排除其他工作中的改动。两场完整游戏沿用原验收，不增加等待或放宽门槛。

| 本地场次 | 请求提前于首帧 | 运行包就绪/首帧后 | Native 就绪/首帧后 | 首步确认 |
| --- | ---: | ---: | ---: | ---: |
| 英文、默认后端 | 8974ms | 1675ms | 25080ms | 108ms |
| 繁中、共享 WebGL2 | 8349ms | 733ms | 52755ms | 147ms |

每场两个独立页面各仅一次 WASM 请求；六套原字体完整缓存 SHA、113/113 HUD、完整行走素材/自然 DOM 帧、首次及重登 Q 开关、两次正常注销与同角色权威位置保持均通过。旧角色/存档和只读源档保持，浏览器、代理、Next、Gateway 全关闭。见 [games.json](games.json)、[preservation.json](preservation.json)。

两场复用真实素材 CacheStorage，清空 HTTP 缓存及认证，并创建新 Warrior；这是既有本地 dca Zone Gateway 配合新前端的资格，**不是公网冷启动，也不是新源码的匹配后端发行**。两场截图已查看，繁中仍有英文欢迎词/NPC 名等旧差异，不计作语言或整体视觉 1:1 关闭。

[英文任务窗](world-r1-first-entry-diary-open.png) · [英文自然行走](world-r1-town-natural-walking.png) · [繁中任务窗](world-r2-first-entry-diary-open.png) · [繁中自然行走](world-r2-town-natural-walking.png)

## 正式服务与后续

前轮 ef6 实际发布后，原公网运行包/Native 120 秒门槛失败，两端已回退；不把本地结果倒填到那次失败。[原失败及回退](../r16-hud-foundation-order-20261116/production/README.md)。本轮重新只读核对的 Web 仍 `20de5ea4fd93b3c61e4aa590dca2ea2d8f5d2339`，固定 Gateway 健康入口仍 `fd3fcedc8b8a4d0b6bb16f6c45cd57a77364c8b5`，均 HTTP200，见 [public-readback.json](public-readback.json)。初次探针误读游戏域名下的健康路径得到404，已按固定控制器的 sslip 健康地址更正；原探针保留在私有记录。

下一轮检查真实公网大运行包/字库正文的缓存与传输：前轮 WASM 为 Vercel MISS/Cloudflare DYNAMIC，约142秒正文；提前8–9秒请求不能推导公网已达标。继续做同源匹配构建和原始公网验收，保留全部六字体/113与8/65/120/20时限。实体手机扫码、三职业完整1–30、地图/技能/阿洛/语言等整体1:1仍开放，Goal active。
