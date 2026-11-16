# CJK 字体持续下载与繁体断点恢复候选

本轮基于 R16 `codex/windows-player-journey` 的 `7de1b38dd26d969bc5daf7cae64261ab70e3285c`。
本地候选 `r16-cjk-progress-d90b62f7` 已验证并保留原改动，**尚未部署**。
整体 Web/Windows 1:1 Candidate、最终人类 Accepted 与 Goal 均未完成。

## 为什么继续处理加载

上一轮补充公网观察保持正式 e0／Fo 不变。其中一场原 Q 抓图辅助观察通过原门槛，
另一场最终能完成游戏，却被原分类器判为失败：Native 在首帧后 **210,624 ms** 才就绪，
超过原 **120,000 ms**。运行包正文为首帧后 69,302 ms，六字体及 113 项最终完整。
不能用功能运行器的 exit 0 覆盖这个失败，也不能将两场不同网络过程解释为截图开关的因果结果。

[保留的原判定和画面边界](preceding-public-observations.json)及
[字体上游时间线](public-e0-failed-font-trace.json)显示：

- SC 初始范围读取一直有已校验进展，仍在首次请求后的 24 秒被总计时取消。
  后续整包中断保留了 54 片，原 e0 的前缀保护确实生效；但期间存在多次整包／来源等待。
- TC 的整包曾收到部分正文，原来没有 TC 分片 pin，后来又完整读取一次。
- 原 Q 黑 PNG 为 3,938 字节，补充被动 screencast 的 22 帧中有两帧全黑。
  输出设备尺寸短暂变化，尚不足以证明 CSS viewport 实际改变或产品黑屏的根因。
  后一场换截图标志时 runtime 阶段不同，不能据此关闭首次 Q 的问题。

这些是正式 e0 的补充观察；不改写[原发行的两场记录](../r16-font-prefix-recovery-20261116/production/README.md)。
本轮没有切生产，因此没有新的上线版本需要回退。

## 改动和不变的验收

两套原字体现在都可逐片恢复：SC 仍为原 **63 × 256 KiB** 单元，TC 新增 **46** 个原字节单元。
最后一片采用实际剩余长度。合并 HTTP 请求仍最多 1 MiB，所有字体共用原来的两个下载名额。
每片验证 SHA，最终字体仍验证完整长度和 SHA；部分字体不会交给 CSS 或共享渲染器。
TC 的整包中断也沿用原配置版本、存储权限、撤销及缓存期限保护。

范围传输改成 **24 秒没有已校验网络进展才退出**；缓存命中、未校验正文和重试不能续期。
另有从实际准入开始的 **48 秒绝对上限**。排队不消耗初始预算，来源响应头仍为 8 秒，
分段正文仍为 15 秒。整包 TC 的后备顺序继续 app-first，范围读取可用已配置的当前发行 CDN。

这改变了内部范围总计时的策略，**没有增加公网 8／65／120 秒或两次 20 秒的原验收预算**，
没有删除任何字体或 HUD 资源。SC 原 SHA、全部 63 片 SHA、缓存键和重启单元保持。
[完整合约与验证汇总](validation.json)。

## 验证结果

- **207 项通过，0 失败／忽略**：144 项 Worker／续传／来源／生命周期回归及 63 项相邻准入、
  游戏运行包预取和扫码准备测试。8 项新增 CJK 测试已包含，未重复计数。
  新测试对旧 Worker 的三个目标用例均失败。
  [144 项](focused-regression.log) · [63 项](adjacent-regression.log) · [旧代码预期失败](old-worker-expected-failure.log)
- **真实 Chrome，实际 HTTP 和 CacheStorage**：旧 TC 中断同条件保留 0 片；新版本保留 43 片，
  浏览器重启且不重发配置后只读剩下 3 片。SC 相邻中断保留 60 片、只补 3 片。
  TC 范围停传保留 42 片，重启仅补其余部分。
- **持续传输**：SC 约 29 秒、TC 约 30 秒完成，均超过旧 24 秒总计时，始终读取范围正文，
  没有改成整包重下。硬上限、停传取消和排队仍有独立测试。
- 五场新 Chrome 用例均核对全部 **六套原字体完整 SHA 与 FontFace 实际解码／绘制**；
  原两个网络名额保持，所有自有浏览器与服务器关闭。
  [六字体实际画面](six-original-fonts.png)及各 `chrome-*.json` 记录。
- 隔离严格 Next／TypeScript 构建通过，42,634 ms，BUILD_ID `Lq1UAo5W5N1biJFOUlIiF`。
  检查 2,181 个已提交 app/lib 文件并保留一个固定生成输入，运行包不变。
  构建后的烟测夹具只补了停传场景的整包后备不可用条件，最终夹具已用实际 Chrome 验证；
  编译的应用和 Worker 未再改变。

| 本地完整游戏 | 首步 ACK | Native 首帧后 | 结果 |
| --- | ---: | ---: | --- |
| 原英文默认流程 | 86 ms | 78,271 ms | 通过 |
| 原繁体 WebGL2 | 已进城并移动 | 未完成测量 | 地形截图等待网页字体，在 12 秒超时；失败保留 |
| 繁体 WebGL2 补充流程 | 110 ms | 18,612 ms | 通过；仅附属地形图改为原始整 viewport CDP 抓图 |

补充流程未改变应用、资源、存档、移动或时间门槛，保留完整行走包及原 65 秒自然可见帧验证，
不用等待网页字体来生成附属截图。它**不能替代或改判原繁体截图失败**。
两场完成的流程均通过六字体、113 项、首次／重进 Native Q、零 Q 世界写入、普通 Alt+X 注销、
同角色落点重登和再次保存。使用自有旧 CacheStorage，并清理 HTTP／登录态；这不是公网冷启动测量。
[英文 Q](english-native-quest.png) · [补充繁体 Q](traditional-native-quest-supplement.png)

15 个私有构建覆盖均已恢复，原 29 份内容 WIP 的原始 SHA 保持，三个游戏流程的浏览器、
代理、Next、Gateway 均已关闭。原账号、认证信息和原始游戏数据未发布到证据目录。
[证据 SHA 与私有回执索引](evidence-manifest.json)。

## 后续门槛

正式仍为 `e0ceea22964c03041d45e49e506b3a8badd6984e`／`dpl_FoNUgjSRpw56hwUFDNrSs7LZZQFN`；
本轮独立 alias、Web `/version` 和 Gateway `/playtest/health` 均读回该版本。
接下来准备本候选的同源 Linux／PG／资产及成对发布资格，再做原两场公网门槛。
新发布控制器必须以 e0／Fo 为上版，不能重跑仍固定旧 6ba／G8 的历史控制器。

发布保护配置现已更新为本候选 `306066f29590d1e2e14155d83644fbc19f11a4c3`，
上版固定 e0／Fo。独立 GitHub 比较核对 3 个提交、62 个文件，两个检查点分别为 37／29 个文件；
Worker 72,815 字节，SHA `5369d5c2a5c51a89e892ad5e283d63b36a2bf0f24cc035b5a3d81cd5e599bbdb`。
**88 项发布保护测试通过**。仅修改发布 pin 及对应精确测试，没有据此切换生产。
[固定配置与源码检查](release/release-pin.json) · [源码闭包](release/source-closure.json) ·
[独立正式读回](release/baseline-readback.json) · [88 项输出](release/release-guards.log)。
同源 [Linux／Postgres CI](https://github.com/Zombieliu/mir2/actions/runs/43782096198) 已启动，
在完成前不认领其结果；后续候选构建、切流和公网两场仍须独立验收。

首次冷 Q、其他线路／地图／断网、三职业完整 1–30、阿洛、语言像素、Windows 真窗口和实体手机继续。
本轮[词库核对](runtime-catalog-observation.json)确认当前 2,242 项 portable 词库已逐字节嵌入已发运行包。
TC 词条中部分英文人名也存在于 Native 作者数据，不能仅凭人名就认定 Web 与 Windows 不一致；
系统欢迎文字及其他显示仍需逐项比对。没有因此关闭语言或整体 1:1。

复测命令（仓库根目录，输出目录需不存在）：

```text
node mir2-web3/apps/web/scripts/generate-font-range-pins.mjs --check
node --test mir2-web3/apps/web/scripts/test-asset-worker-font-ranges.mjs mir2-web3/apps/web/scripts/test-asset-worker-cjk-progress.mjs
node mir2-web3/apps/web/scripts/smoke-asset-font-ranges.mjs --output <new-output> --font tc --failure-mode whole-prefix --chrome <Chrome-executable> --playwright-package <package.json-containing-playwright>
node mir2-web3/apps/web/scripts/smoke-asset-font-ranges.mjs --output <another-output> --font sc --failure-mode progressing --chrome <Chrome-executable> --playwright-package <package.json-containing-playwright>
```
