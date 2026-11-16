# 共享渲染器与CSS的完整字体下载通道

2026-11-14，产品提交 `6ae96fa6017d511255ac46216c034594b6fe500f`，R16。上一轮姓名／死亡文案候选因公网字体504、共享HUD未就绪而[回退](../r16-name-autosize-20261114/production/README.md)。本轮确认页面预热器的两路限制没有约束渲染器直接发出的六套字体。Worker现对每个完整字体源的读取共用两条通道，直到正文完整或该源失败才释放；排队时间不计入网络响应头预算，直接调用者取消时移除尚未开始的读取。普通图片、范围读取、同字体共享与缓存命名空间保持既有规则。

原 app／普通源7秒、preferred源8秒响应头、15秒正字节idle、32MiB上限及120秒Native门槛均未提高。Worker为43,171字节，SHA `3b07de456c113135d2d37248893a707ce98179b419429fe6d58c18981b510dba`；原六套Windows完整字库和固定资产／运行包保持。此改动没有部署Gateway、修改角色或账本。它补齐下载并发约束，不能单凭本地结果断言全部公网504已有唯一根因或已解决。

## 原始失败、检查与实际Chrome

三个新增执行路径检查在原Worker上0过／3失败；改动后的Worker、缺失重试、生命周期和字体预热105项，以及相邻预取27项共132项通过，零skip。保留[修前日志](before-tests.txt)、[105项](after-tests.txt)、[27项](adjacent-tests.txt)及[原件SHA／显示规范](log-provenance.json)。来源分别是 `test-asset-worker-fallback`／`missing-retry`／`lifecycle`／`bevy-runtime-font-prewarm` 和 `test-asset-prewarm-policy`／`request`。

真实Chrome在相同HTTP服务上同时请求原六套字体及重复繁中字库；服务可同时传送两份字体正文，先给32字节再延后2秒完成。原Worker只完成2/6，其余四项503；新Worker完成6/6，均实际FontFace解码、完整长度／SHA和CacheStorage校验。两条字体正文仍活跃时，普通HUD图片也完成；重复繁中仅一次上游读取。该服务是可复测的受控拥塞，不是公网性能测量。[修前false](browser-before.json) · [修后true](browser-after.json) · [原字体画面对照](fonts-before.png)／[修后](fonts-after.png)。现有当前源503＋旧源404的下一次合法读取恢复检查也用真实Chrome完成六字体及绘字。[混合源恢复](browser-source-retry.json)

## 独立构建与完整游戏

生产构建35.898秒、BUILD_ID `og1FaBw6WVLshiPDpJG5y`，2,773项输入逐字节对照提交：全部非public Web源、12项GDI public输入、新Worker和共享键盘；仅声明固定运行包清单转换。原Shell动画与Native WIP未混入。[源码／构建绑定](source-qualification.json)

默认渲染英文及真实共享WebGL2繁中分别完成冷启、普通注销、关闭全部该浏览器进程、同角色重登，共四会话。原120秒Native门槛下，默认冷／暖54.548／1.287秒，共享WebGL2冷／暖52.244／1.254秒。每场完整六字体长度／SHA、113/113、共享HUD和Q通过；真实 app 繁中字库先给1024字节后停止15秒，再从镜像取完整原字体，零前缀入缓存，Worker停启后仍用原缓存。保留既有场景、NPC、运行包和逐PNG延迟故障。[完整四会话](local-four-sessions.json)

两路还完成实际NPC1同ID交谈／原生X关闭、狩猎区零步提示、补给商人长路线、停止和权威保存。目标 `(324,291)` 的正常NPC接近半径为2，最终 `(322,293)`；逐段前进仍用原8秒限制。冷场保存 `(289,616)` 与重登权威初始坐标相同；两次注销都回选角。全部本轮浏览器、代理、Next、Gateway已关闭。原30级黄金夹具保持，不能算新的三职业新手至30级验收。[默认HUD](default-en-hud.png) · [默认到达](default-en-vendor-arrival.png) · [共享WebGL2 HUD](shared-gl2-tw-hud.png) · [繁中到达](shared-gl2-tw-vendor-arrival.png) · [root审图及SHA](visual-review.json)

首次准备脚本错误假设旧bootstrap索引有四个wrapper，实际三个，尚未启动游戏浏览器即停止。原失败脚本保留；新fixture只机械重绑定16个冻结观察器的源码／构建／路径／父提交，并逐字节核对16个standing和三个bootstrap wrapper。原独立Walking门槛保留，未改变任何游戏、超时或坐标条件。[准备失败及更正范围](retained-preparation.json)

## 发布与剩余范围

发布保护固定准确源码和三个Git blob，并将新Worker的完整字节／SHA加入原14项protected/public校验，共15项；68项发布检查通过，含各项损坏拒绝和公开校验失败回退。[检查](release-tests.txt)。[04:10UTC的正式基线](production-readback.json)保留。随后已尝试发布，15项protected/public字节校验通过，但公网共享HUD未就绪、字体0/6，完整功能资格false，已回退e6c923；运行包在首帧后133.443秒才完整到达，原120秒资格也false。两路字体机制的本地通过不能冒充公网修复；先做实际响应头／正文及Worker隔离诊断。[失败、回退与下一步](production/README.md)。

公网冷加载、实体手机相机扫码、真实第三格多人占位、R15旧停步、英文攻击模式／货币行夹中文、raw NPC标题和繁中名字分行、完整死亡复活、全部地图技能与GPU实体、三职业完整当前路线、Windows窗口和最终人类Accepted仍开放。此候选继承已验证的姓名及死亡文案代码；本轮没有重做十语死亡组件40图。整体1:1 Goal active。
