# R16 Worker重启后恢复缓存：本地候选

2026-11-13，以下保留发布前本地候选范围。随后已正式发布f6acaad6b0：公网缓存恢复分项通过，但共享HUD因字体/皮肤失败未就绪，完整试玩仍false；见[正式发布与保留失败](production/README.md)。整体1:1未完成。

## 触发与修复

浏览器可停止闲置Worker，注册和CacheStorage仍保留；旧实现的配置与缓存分区索引只在内存，重启后回到bootstrap，直接绕过已经成功缓存的HUD图片。在真实Chrome中，同一66374字节原版Prguse/1.png，停止前200，停止后变为原站504。[修复前](before-browser.json)。

现在配置、镜像、字体策略和分区索引保存在独立元数据缓存；新Worker首次相关请求只恢复一次，等盘最多750ms，新CONFIG可立即中断旧恢复，原站路径不会因存储故障永远等待。所有快照写入按序；CONFIG/RESET使迟到读取失效，RESET清除元数据并禁止HINT再次持久化，直到明确的新CONFIG。动态HINT按新增批次保存，已有规范索引不重复写。响应与缓存写分别持有FetchEvent生命周期，前景不等缓存落盘。

修复后真实Chrome确认stopped→running，同一图片字节和SHA完全相同，**0次新增原站/镜像读取**；隔离游戏中停止Worker也通过。[修复后](after-browser.json)。没有改字体、WASM、战斗、权限、奖励或存档。

## 验证

- 147项通过，包含17项新生命周期用例、43项已有Worker用例及预取、运行包、启动/移动端降级和发布合同。旧启动预算测试的内部变量名断言在原HEAD上已过时；移除该条断言，保留降级条件和既有行为取消/时限测试。[测试](tests.log) · [修复前8失败/1通过](before-unit-red.log)。
- 隔离生产构建通过；55份输入逐文件校验，运行包仍bevy-84ef836460a84e03，固定素材仍20261108-r16-08ec86d5105c。[构建](build-receipt.json) · [源码绑定](source-binding.json)。构建基于0bb62d2fc9，后续容量维护未改Web/客户端/游戏数据；补充的可移植浏览器QA脚本独立复测。
- 默认WebGPU与共享WebGL2均在原120秒Native门槛内就绪，原版6套字体完整SHA、113/113、繁中共享Q、8次关Q即移动、真实Worker停止/重启、正常注销/落盘、彻底关闭Chrome后同档再进与再次注销通过。原始存档未变，自建浏览器/Proxy/Next/Gateway均关闭。

| 路径 | 首可操作后等Native（受控源） | 完整Chrome重开后等Native | Worker重启缓存 | 两次保存 |
| --- | ---: | ---: | --- | --- |
| 默认WebGPU | 41.293秒 | 1.488秒 | 同图200，0次原站读取 | 通过 |
| 共享WebGL2 | 41.360秒 | 1.472秒 | 同图200，0次原站读取 | 通过 |

[默认重启截图](compatibility-after-worker-restart.png) · [共享WebGL2重启截图](webgl2-after-worker-restart.png) · [完整分项](receipt.json)。

素材使用固定原版字节的本地镜像，并有故意的原站/响应头延迟，**不是公网冷加载数字**。两种运行包均实际读完整正文并校验摘要：[默认](compatibility-runtime-body.json) / [共享WebGL2](webgl2-runtime-body.json)。早期r1/r2/r4因取证未取得完整运行包或调试缓冲逐出而失败，保留摘要及原本机回执SHA；调整有界取证缓冲、验证实际首尝试与后备来源后才通过，未放宽Native120秒或正文/字节检查。

## 复测入口

从仓库根运行：

```text
node --test mir2-web3/apps/web/scripts/test-asset-worker-lifecycle.mjs mir2-web3/apps/web/scripts/test-asset-worker-fallback.mjs
node mir2-web3/apps/web/scripts/smoke-asset-worker-restart.mjs --output <新的证据目录> --playwright-package <已安装playwright的package.json> --chrome <本机Chrome或Chromium路径>
```

浏览器脚本使用独立隐藏Chrome和随机loopback端口，只读取原图与Worker，不登录、不访问公网、不控制玩家已有标签页。可用`--worker-source`重放旧源码，`--asset-file`指定同一完整图片；写回执使用wx避免覆盖。

## 剩余边界

真实公网原19.7秒首步与232.2秒HUD、此前120秒冷启失败仍未解决；Worker已配置时的7个来源串行上限49秒与页面约30秒预算错位独立追查。旧health在首步后0.8秒内出现一次472微秒成功准入，但该累计计数不能无条件归属这一步；已有owner优先队列，尚不能把长尾写成个人Tick或普通软件FIFO已定位。继续获取被动WS帧/发送链路证据。

截图仍可见既有NPC名字碎片、任务夹杂英文；本轮不算全语言像素、三职业1–30或全部地图/技能/阿洛/Hosted/Windows窗口完成。实体手机与最终人类Accepted保持开放。原65份未提交文件逐字节保全；未改DNS、官网、网关部署或生产角色存档。

仓库日志仅规范行尾与空白；原始本机日志保持不变，摘要见receipt.json的logOrigins。
