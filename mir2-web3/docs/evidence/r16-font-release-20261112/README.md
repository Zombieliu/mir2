# R16 字库并行与提前读取正式发布

2026-11-12，正式入口：[numeron.obelisk.build/game](https://numeron.obelisk.build/game)。曾切换到候选，但**公网完整冷加载验收未通过，已回退到72c00d20ae**。

- 游戏源码：`df7aace1692ccbd28291a917b7e74eafe6942940`，发布辅助程序：`7a7e428abed58a4a19f7adc5496238f094738043`。
- 正式部署：`dpl_AX8yVT3mRrwfAmMSZmv5NfZi2DgQ`，从旧版 `72c00d20ae` 切换。
- [Stage 43222240745](https://github.com/Zombieliu/mir2/actions/runs/43222240745) 与 [Promote 43223308299](https://github.com/Zombieliu/mir2/actions/runs/43223308299) 均成功；切换同一生产构建。

本版包含字库与运行包重叠读取、当前版本的配置字体镜像优先于应用源，以及两份大字库提前启动。六份原字体、Unicode回退、两任务并发、正文/超时门槛和原运行包不变。部署前133项、生产构建、两后端任务窗/8次立即移动/重启保存见[候选时序证据](../r16-startup-timing-20261112/README.md)。本次68项发布检查通过，53份本机构建覆盖源码与提交匹配，65个原有文件字节保全。

## 公网已通过的分项

固定源码云端构建通过；切换前后保护验证入口、二维码会话和11个启动素材。独立读回 `/version`、`/game`、Worker与6个小型JS/WASM文件，共9条路径，大小和摘要匹配；Worker为31615字节，SHA-256 `d86af7087c4701e020847fff0e22d9014b0deef4175d3dd81b69c48ad5688005`。

普通Chrome简中、繁中均实际渲染二维码、到待确认并取消。第二轮完整尝试中，官方钱包手机尺寸浏览器一次性Testnet虚拟Passkey登录签名、普通新测试角色进入比奇、物理Arrow一步获得权威回包、繁中备用Q及正常退出通过。点击进入游戏到首可操作画面约 **16.1秒**，只是本次样本。没有资金转账、真实玩家钱包使用、生产存档迁移、网关/DNS/官网项目变更。原始认证记录仅本机保留，仓库仅存筛选后的验收字段。

## 未通过的分项与边界

1. 首轮在官方钱包首次导航的45秒DOMContentLoaded门槛超时，零钱包创建/签名、零游戏角色，所有自有浏览器关闭。[原失败](first-wallet-navigation-failure.json)保留。另一个只读iPhone13新页面探针约4.8秒显示钱包标题，设备与Pixel7验收不同，仅用于诊断。第二轮使用相同14份执行脚本、相同期限/门槛及新浏览器上下文。
2. 第二轮未观测到 `entry-monster-standing` 的完整版本包正文，视觉包门槛失败，需继续核对实际包路由、绘制与观察链路，不能直接由此声称所有怪物未显示。
3. 原WebGPU运行包请求收到200，正文仅观测到10391576字节后 `net::ERR_ABORTED`；额外180秒范围内共享HUD仍未就绪，未取得共享Q通过证据。
4. 当前会话HUD预取为 **108/113**，失败项是 `Prguse/1.png`、`4.png`、`6.png`、`12.png`、`13.png`。普通备用Q通过不能替代共享HUD和完整预取。

实体手机摄像头未测试；本次为官方钱包页面及浏览器设备模拟。钱包来源由执行器预检固定为 `c74a27a48d57a6dd14cfc0ccfcf47e68ab58163b`；原onboarding回执中旧版信息字段不作为当前来源证明。

- [正式部署、通过与失败分项](receipt.json) · [提交/构建绑定](committed-source-binding.json)
- [Stage](stage.json) · [Promote](promote.json) · [独立公网文件核验](final-public-readback.json)
- [公网备用任务窗截图](public-quest-fallback.png) · [68项发布检查](release-tests.log)

此前故障应用源120秒冷启失败仍见[原记录](../r16-font-overlap-20261112/README.md)，未被本机或公网分项成功覆盖。[回退43226868995](https://github.com/Zombieliu/mir2/actions/runs/43226868995)通过，别名与/version均读回旧版72c00d20ae；[回退回执](rollback.json)。后续候选将新增字体预取延至运行包完成之后，以免预先占用其下载带宽，再重做验收。整体1:1、完整三职业当前路线、其他地图/窗口、NPC文字图层、纯GPU、Hosted和最终人类Accepted继续开放。

回退后对失败的5份HUD图片进行10次只读正文核验，应用源与CDN均返回200、相同PNG摘要，约0.6–1.3秒；见[空闲读回](idle-hud-image-probe.json)。文件存在，不代表启动并发时浏览器已经成功读取，也不能据此确定全部失败根因。
