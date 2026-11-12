# R16 字体临时失败后真正重试：本地候选

2026-11-13，候选通过下列范围，尚未发布。公网仍`f6acaad6b0`，前次[共享HUD字体/皮肤失败](../r16-worker-restart-20261113/production/README.md)保持false。

## 真实问题与修改

公网25次页面字体读取全404，6套字体均受影响，其中12次在12ms内返回。Native最后font/skin failed，共享HUD未接管。[安全观测与原始证据摘要](production-font-observation.json)。跨域PerformanceResourceTiming受TAO限制，不能用responseStart=0证明每个源都没响应头；具体上游状态及是否另有skin失败不能仅从这些时序确定。

实际Worker存在可复现的判定错误：当前版本暂时不可读，旧版本缺少文件返回404，便按逻辑URL负缓存10秒；随即两次重试直接404。现在只有**全部已尝试候选均明确404/410**才确认缺失；任何连接失败、超时、不完整字体或其他HTTP错误都不能据此负缓存。保留首个有效错误的正文和Retry-After，无法获得HTTP响应时返回可重试504。成功响应仍优先；合法Range206不变。优先读取过的同版镜像仅在本次调用按精确URL去重，下一次正常重试仍可访问；同时避免以普通读再次绕过完整字体校验。

没有改字体、WASM、素材、规则、权限、存档、字体大小限制或时限。缓存schema仍sw8，仍用原字节；不是删字/替换字体。

## 复现与通过范围

- 最初11项回归在旧代码上7失败/4通过；随后补充空正文与无Range206两项边界，共13新用例。连同已有Worker43、生命周期17、真实Page字体预取与完整预读等共**101通过，0失败/跳过**。[测试](tests.log) · [保留红结果](before-unit-red-11.log)
- 实际Chrome、真实ServiceWorker和六份原版字体：当前源先503、应用源/旧源404，旧版只完成5/6且繁中FontFace加载失败；新版下一次正常重试取到200，6/6、29517096字节全部缓存SHA一致，繁中真实渲染。无公网网络、账号或手工预填缓存。[修复前](before-browser.json) · [修复后](after-browser.json) · [实际字体截图](original-traditional-font.png)
- 隔离生产构建通过，58份受限覆盖输入逐文件核验，原六份Bevy运行包及三套小型Rust模型未变。[构建](build-receipt.json)
- 隔离正常新手角色，以原SC字库模拟当前503→应用404→旧404→当前完整字体恢复；两条实际渲染路径均通过原120秒Native门槛、6套完整原字体、共享HUD/繁中Q、113/113、8次关Q即物理移动、Worker真实停止/重启及同缓存图片、普通注销落点落盘、全部Chrome进程退出后同档重进和第二次普通注销。原毕业控制角色及源File档均未变；**没有重新跑1级至30级完整路线**。

| 路径 | 首可操作后等共享Native（受控源） | 完整浏览器重开后 | 共享任务窗/113项/保存 |
| --- | ---: | ---: | --- |
| 默认WebGPU | 41.280秒 | 1.451秒 | 通过 |
| 共享WebGL2 | 41.361秒 | 1.467秒 | 通过 |

[默认实际画面](default-native-after-restart.png) · [共享WebGL2实际画面](webgl2-native-after-restart.png) · [完整分项与原回执摘要](receipt.json)。两种运行包均保留完整正文/摘要证据：[默认](default-runtime-body.json) / [WebGL2](webgl2-runtime-body.json)。

## 复测与下一步

从仓库根：

```text
node --test mir2-web3/apps/web/scripts/test-asset-worker-missing-retry.mjs mir2-web3/apps/web/scripts/test-asset-worker-fallback.mjs mir2-web3/apps/web/scripts/test-asset-worker-lifecycle.mjs mir2-web3/apps/web/scripts/test-bevy-runtime-font-prewarm.mjs mir2-web3/apps/web/scripts/test-asset-prewarm-request.mjs
node mir2-web3/apps/web/scripts/smoke-asset-font-retry.mjs --output <新的证据目录> --playwright-package <已安装playwright的package.json> --chrome <本机Chrome路径>
```

浏览器脚本仅新建自己的隐藏Chrome与loopback HTTP，无账号/公网访问；用`--worker-source`可读旧版Worker重放失败。只发送正常缓存配置，不把字节预填CacheStorage。

下一步发布该候选并复验真实公网共享HUD及冷加载。受控41秒/1.5秒不是公网保证，也不覆盖原120秒冷启和18.635/19.7秒首步失败。截图中仍有既有NPC名字碎片和中英混合，语言像素、其他地图/技能、阿洛、三职业当前1–30、Hosted/Windows窗口、实体手机和最终Accepted均保持开放。全局1:1 Goal继续。

65份原WIP逐字节保全；本轮没有改公网网关、DNS、官网、钱包/付费或生产存档。仓库日志仅规范行尾/末尾空白，原日志摘要保留于receipt。
