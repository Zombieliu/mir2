# R16 字库恢复与运行包并行读取

2026-11-12（Asia/Shanghai），在 `codex/windows-player-journey` 的 `958f5aca2a38128fb39b245b30ec7b8a6d3b0925` 上保留原 WIP，完成本机候选 `958f5aca2a38-r16-runtime-parallel-fdc43e97`。本轮未发布；核验时公网仍为 `cd6410be749412d608a538e2db442b7e4d519de0`。整体 1:1 Goal 保持 active，完整候选验收未通过。

## 代码与边界

固定 Hosted 资源版本优先读取同版字库镜像，8 秒响应头未到或正文连续 15 秒无非空数据时回到应用入口；持续有数据的原版字库不按总时长截断。换源前完整读取，避免混用半截正文；保留 MIME，按解压后正文修正长度和编码，缓冲上限 32 MiB。开发环境沿用源站优先，旧版镜像、Range 和普通图片顺序保持原样。

运行脚本与 WASM 同时读取，二进制流编译后仍由原 wasm-bindgen 入口实例化，等待原固定 Canvas；错误 MIME 或不支持流编译时用同一份正文编译。二进制原 120 秒取消预算保留，导入失败取消本轮下载，失败的初始化 Promise 释放以便后续调用再试。全套原版字库、语言、运行包和服务端规则未更换。流编译与实例化分离符合 [MDN 接口说明](https://developer.mozilla.org/en-US/docs/WebAssembly/Reference/JavaScript_interface/compileStreaming_static)，三份固定运行包的本地 glue 均核对支持 WebAssembly.Module 输入。

## 已验证

87 项字体/Worker/地图相邻检查、24 项运行初始化/包选择/网络策略检查通过。旧字体路径在新增三项回归上按预期失败。最新生产与 TypeScript 构建通过，21 个源码覆盖项和 27 个复用输入逐项核对；未重建 Rust 运行包。React 检查涵盖独立请求并行、失败取消、Promise 去重与失败释放；组件 Hooks、Canvas 归属和输入规则未改。

真实普通 Chrome、两个实际 HTTP 源站、原版 SC 字库 16,437,364 字节。旧镜像发出 1 KiB 后停传，18 秒仍未完成；修复后约 15 秒换源并完整完成。持续传输约 19 秒的正文正常完成；断流和 gzip 正文通过。CSS 与第二个消费者共享一次读取，刷新使用缓存；四种成功条件的截图字节完全相同，原字体 SHA256 为 `2c76254f6fc379fddfce0a7e84fb5385bb135d3e399294f6eeb6680d0365b74b`。[实际文字](font.png) · [完整回执](receipt.json)

以上为受控传输结果，不是公网提速倍数。默认系统网络的整游戏验证使用隔离 File、正常登录建角与真实生产构建，未拦截请求或绕过 TLS；保留源站 SC 正文停传和地图两次 503/17 秒晚头条件。

## 整游戏通过与失败

| 运行 | 配置 | 整轮 | HUD 观测 | 结果原因 |
| --- | --- | --- | --- | --- |
| game/quest-after-r1 | compatibility | 失败 | ready | The expression evaluated to a falsy value: |
| game/quest-after-r2 | webgl2 | 失败 | 未就绪 | page.waitForFunction: Timeout 120000ms exceeded. |
| game-overlap/quest-after-r1 | webgl2 | 失败 | 未启动 | page.waitForFunction: Timeout 120000ms exceeded. |
| game-parallel/quest-after-r1 | webgl2 | 失败 | 未就绪 | page.waitForFunction: Timeout 120000ms exceeded. |
| game-parallel/quest-after-r2 | compatibility | 失败 | 未启动 | page.waitForFunction: Timeout 120000ms exceeded. |

最新共享 WebGL2 的脚本与 WASM 请求起点相同；脚本约 43.214 秒完成，WASM 约 90.737 秒完成，运行实例随后启动，但简中字库仍未在门槛内到齐。最新默认 WebGPU 的两请求也同时发出，观察结束均未收到响应头；繁中字库在约 118 秒时换回本机源站并完成。这些是各次默认系统网络的实测，不能用来承诺固定公网提速。

前置 body-only 默认游戏确实到达共用 HUD、任务窗 ready，并经过六份缓存正文校验；但 Worker 时序记录为空导致随后断言失败，未完成 Q、移动和注销，整轮仍记失败。后续将来源验证改为实际浏览器 Worker 回包、六份缓存哈希及零 SC 源站请求；Native 120 秒门槛保留。运行包并行方案另核对脚本和二进制请求起点相差少于 1 秒。

并行预取六份字库的中间原型未通过整游戏；多次镜像头超时、回到被停住的 SC 源站，且运行包未就绪。该原型已从项目源码与构建输入撤回。其代码、失败回执，以及两次浏览器验收脚本准备错误均原样保存在 `C:/r16-font-body-progress-20261112`；本页 JSON 使用白名单字段，不含账户凭证、钱包会话或认证状态。

## 保全与继续

原 62 个文件与 38 条 WIP 状态由字节哈希校验保留，其他任务的容量文件未纳入提交。继续处理 Native 字库到齐与完整冷加载，核对真实源站和镜像路径、错误恢复及两种生产渲染路径；公网首步长尾、现版三职业 1–30、其余玩法、实体手机、Windows 窗口及最终人类 Accepted 仍开放。

下一轮先顺序核对普通浏览器下当前应用入口与固定镜像的完整正文、缓存和传输时序，并补齐运行脚本长期等待时的退出与恢复。沿用现有系统网络和验收门槛，不把静态直连诊断当作普通玩家验收。
