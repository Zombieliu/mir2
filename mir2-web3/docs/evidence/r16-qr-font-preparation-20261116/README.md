# R16 二维码后预取原字库（2026-11-16）

后续发行：应用 `6ba38557dd` 已完成匹配打包和两场原门槛公网验收并保留线上版本；R2 Q 实际为备用窗口，整体 1:1 和实体手机仍开放。见 [发布回执与界面分支](production/README.md)。以下本地验证与当时的未发布状态保留为历史记录。

桌面玩家看到二维码后，已有资源 Worker 可以提前填充完整简中字库；进城读取复用同一份缓存填充。扫码认证协议和普通玩法保持原有入口，**本轮为本地候选，尚未发布**。

源码基于 `1f08bfe4be38cb25ef8f3656048863a187cef8da`，候选 `r16-qr-font-8515912b`，构建 `lj7I4rCtgpbaostdWbw5n`。对应 [source.json](source.json)、[build.json](build.json)、[compiled-marker.json](compiled-marker.json)。

## 为什么做这一项

实际安装的 Chrome 用全新独立配置、每次只读取一个公网资源。游戏域名运行包原体31,452,436B，在SIN的Vercel MISS读取约9.18秒，随后 `WebAssembly.compile` 约25ms；同体经资源CDN HIT约54.16秒，SHA均为原锁定值。现有产品流式编译路径约6.75秒，但该流式场只直接观察完整字节数/有效模块，不声称单独实测了该场SHA。此实验未实例化App或测GPU/游戏负载。

两个原CJK字体的独立完整下载如下，不是稳定吞吐量保证：

| 来源 | SC原16,437,364B | TC原11,941,968B |
| --- | ---: | ---: |
| 游戏域名 | 87.97s | 66.83s |
| 当前资源CDN | 106.99s | 27.15s |

四场字体整份SHA全部通过。这说明不能仅由HIT/MISS推导下载快慢，不能将此前142秒长尾直接定因为编译或缓存未命中。证据见 [运行包](diagnostics/wasm.json)、[字体](diagnostics/fonts.json)、[冻结诊断输入](diagnostics/source-record.json)。冻结脚本保留当时主机路径，是审计记录，不能直接当作跨主机产品脚本。Cloudflare Cache API内容按数据中心保存，也不能用一个节点的命中推导全网已暖：[官方Cache API](https://developers.cloudflare.com/workers/runtime-apis/cache/)。

## 本轮行为

- QR图像完整解码并获得绘制机会后才通知Page；事件只有递增编号和阶段，不包含登录URL、意图或凭证。
- 只对仍有效的待确认QR、允许高性能运行时的设备预取；触屏兼容/显式禁用、过期QR会跳过。只有同源、正确脚本、已确认配置的Worker可接管；最多后台等8秒确认，不阻塞二维码/确认/登录。
- QR取消、刷新或组件卸载会退役自己的前台读取，旧事件不会停止新读取。Worker现有的不可变字体共享填充可以继续供其他读取者/进城消费；不声称取消后所有共享网络工作都停止。
- 使用已有原SC整字体/63分片校验及缓存流程，只提前时机；其余五字体和113HUD仍按原路径加载。没有改字体、Glyph覆盖、权限、模型调用、游戏等待门槛或缓存命名。

185项定向/相邻检查通过，失败/跳过0；其中14项新行为和24项钱包认证回归已包含，不能再相加。实际 `npm run test:bevy-runtime-startup` 的122项也是上述子集。[测试记录](tests.json)

Chrome loopback使用实际Page/QR effect源码、真实二维码矩阵、当前Worker和原16MB SC字体：二维码前零提前读取，取消前台后进城读取接管，同一体仅16次Range；63原分片与完整字体SHA均通过，原字库 `FontFace` 实际加载/中文绘制通过；确认前取消的另一场零字体请求。[浏览器记录](chrome.json) · [二维码可见](qr-visible-before-font-completes.png) · [原字库绘制](shared-font-complete.png)。这是组件/Worker夹具，**不是手机确认或完整游戏证据**。

## 完整本地游戏

严格Next/TypeScript构建通过（43262ms），核对2180份既有app/lib、1个新增模块和1份原生成输入，34份固定输入，排除其他WIP。英/繁两整局在原流程前增加一次实际编译QR组件的本地待确认夹具并立即取消；不等待预取完成、不放宽原8/65/120/20或全资源断言。随后使用普通密码和Gateway协议创建新Warrior、进城、移动、Q、正常注销、刷新重登同角。

| 本地场次 | 字库开始提前于首帧 | Native就绪/首帧后 | 首步确认 |
| --- | ---: | ---: | ---: |
| 英文、默认后端 | 14206ms | 10519ms | 98ms |
| 繁中、共享WebGL2 | 14115ms | 4586ms | 113ms |

两场均真实出码、取消后零继续轮询；六字体完整缓存SHA、113/113、原行走包/自然DOM帧、首次和重登Q开关、两次普通注销、权威位置保持/旧档保全通过。复用真实素材CacheStorage、清除HTTP缓存及认证，因此不能据此宣称公网冷启动已过或作因果加速比例。二维码只收到本地pending夹具，未进行真实钱包签名/实体手机确认。[games.json](games.json)

[英文实际QR](world-r1-qr-pending-actual-component.png) · [繁中实际QR](world-r2-qr-pending-actual-component.png) · [英文任务窗](world-r1-first-entry-diary-open.png) · [繁中任务窗](world-r2-first-entry-diary-open.png)

29份原WIP逐文件原SHA保持；8份私有源码覆盖恢复，浏览器/代理/Next/Gateway全关闭。[preservation.json](preservation.json)。React事件/Hook/清理/可访问性复核见 [react-review.json](react-review.json)。

## 仍待完成

本轮只读复核正式Web仍20de、Gateway仍fd3，均HTTP200：[public-readback.json](public-readback.json)。前轮ef6公网超时及两端回退继续保留，不能倒填为本轮通过。[原失败](../r16-hud-foundation-order-20261116/production/README.md)。

以本轮提交做匹配发行构建，再做原始两场公网/真实钱包流程。另已定位1f08普通CI的Windows超长路径checkout和既有Rust格式失败，仍开放，不能称全部Candidate CI已绿：[ci-audit.json](ci-audit.json)。三职业完整1–30、地图/技能、阿洛、语言像素/混合文本、Windows窗口及实体手机/最终人体验收均继续；整体100% Candidate/Accepted均未完成，Goal active。
