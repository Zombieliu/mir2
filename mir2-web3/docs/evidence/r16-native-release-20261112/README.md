# R16 输入与资源恢复修复正式发布

已发布源码 `72c00d20aeb82a9f87c79c2a8567a3146d9a2cfc` 至 https://numeron.obelisk.build/game，部署 `dpl_DMT1cgUABySkvpn6VQEvAXD7j3dL`。发布实现 `39e8bcaeec33712491db690ed947b07116f763de`；正式流量原为 `888dd59046` / `dpl_CWdmzkhCR1isr22yeXM3o5WekiwB`。本次只更新游戏前端。

## 发布和核验

- [Stage](https://github.com/Zombieliu/mir2/actions/runs/43203274456) 与 [Promote](https://github.com/Zombieliu/mir2/actions/runs/43204443601) 成功。核对游戏项目、固定素材凭证、源码闭包、受保护页面、扫码中继及 11 个启动包；流量映射到该候选，没有回滚。
- 发布配置 68 项检查通过。匹配构建的 51 个覆盖文件及 7 个本轮源码文件均按 Git blob 绑定；此前本机 121 项检查、TypeScript、生产构建和两后端真实浏览器行为验证见[候选证据](../r16-native-input-recovery-20261112/README.md)。
- 线上 `/version`、`/game`、Worker 及六个小型 JS/WASM 文件共 9 个地址读回通过。Worker 为 31,322 字节，SHA256 `a3ee48352fd28d992d677b68ec17867280fb68895794fbff2800c4f70e759e71`，匹配候选。
- 普通 Chrome 简繁中各一次首次点击真实出码、等待确认、取消后停止轮询通过；无浏览器请求拦截或玩家命令。
- 手机尺寸官方钱包浏览器用一次性 Testnet 虚拟 Passkey 完成一次登录签名，普通建角、比奇首帧、从 (288,616) 物理单按 ArrowRight 到 (289,616) 的权威回包、共享 WebGPU HUD／繁中任务窗和 Q 关闭通过；113/113 预取、两帧原任务标记、正常注销回选角及自建浏览器关闭通过。整轮 signed smoke 为 true，无转账或充值。
- 本轮点击进游戏到首个可玩画面约 22.8 秒，共享 HUD 被观察到约 138.3 秒。约 185.1 秒的观测起点到首帧还包括钱包确认和建角，不混作点击后的加载耗时；移动包到服务器确认约 142 毫秒。

## 保留失败与范围

首两次额外 curl 读回分别有一个 presentation WASM 请求和一个 `/version` 请求失败，旧脚本未采集失败状态/错误详情。独立重读与后续完整 9 个地址均通过，文件摘要匹配清单；原失败另存，不记为首次全通过。

候选曾在真实 CDN 两次 WASM 正文读取中达到原 120 秒期限，未完成冷启动；这些失败完整保留。单独读取同一固定包约 66.1 秒，6,431,323 字节压缩流解码为 31,441,753 字节，SHA256 匹配 `e66e0b11416233879b0a0c1e9778a86253c532b8e60fd4c5d8339e20264c1c82`。本次发布不修改或放宽运行包启动期限，后续继续处理资源传输长尾。

实体手机相机、完整三职业 1–30、纯 GPU 实体、其他窗口、Hosted、Windows 窗口、语言像素及整体 100% Candidate 继续开放。手机尺寸官方钱包浏览器和一次性 Testnet 虚拟 Passkey 不能代替实体手机验收；未转账或充值。

## 保全

原 62 个文件摘要及三个原动画绘制候选完整保留，未纳入发布；Shell 只提交本次键盘补丁。未部署 Gateway、修改 DNS、官网、原生产存档或链上合约。

[发布与试玩分项](receipt.json) · [源码与构建绑定](committed-source-binding.json) · [线上文件摘要](final-public-readback.json) · [首次读回失败](readback-first-failure.json) · [Stage 原回执](stage.json) · [Promote 原回执](promote.json) · [公网共享任务窗原图](public-shared-quest.png)
