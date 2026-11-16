# 批准交接的被动时序与 e0 两轮对照

本轮增加 QA 观察工具，没有修改应用行为或发布。正式网站和 Gateway 独立读回仍为 `e0ceea22964c03041d45e49e506b3a8badd6984e`，域名仍指向 `dpl_FoNUgjSRpw56hwUFDNrSs7LZZQFN`。两轮都是现有 e0 的独立对照，不能替代之前 f05 的失败验收。[HTTP 读回](independent-http-final.json) · [域名](independent-alias-final.json) · [f05 原失败与回退](../r16-diary-nav-handoff-20261116/production/README.md)

用户路径是普通浏览器 `/game` 出码 → 官方手机尺寸 Testnet PWA 内一次批准 → 第一方状态轮询／游戏交换 → 普通协议建角、进城、移动和退出。使用虚拟 Passkey，未覆盖实体手机相机。保留原 8／65／120 秒、完整六字体和 113 项门槛，没有主动状态探针、拦截、额外网络参数或焦点切换。每轮 25 份脚本均冻结并核对 SHA。[R1 输入](r1-frozen-qa-inputs.json) · [R2 输入](r2-frozen-qa-inputs.json)

| 原流程结果 | e0 R1 | e0 R2 |
| --- | --- | --- |
| 官方钱包单次批准、游戏交换／LoginSuccess | 通过 | 通过 |
| 批准正文完成 → 下一次轮询完整正文 | 1,399ms，approved／有证明 | 1,653ms，approved／有证明 |
| 批准正文完成 → 游戏交换完整正文 | 1,754ms | 2,200ms |
| 首个可操作场景 | 原门槛未观察到 | 第 103,455ms 出现首帧 |
| 实际首步 | 未到达，Walk 0 次 | 物理输入到权威回包 7,692ms，通过原 8 秒但延迟明显 |
| 首次任务窗 | 未到达 | React 备用界面，Native 任务窗未通过 |
| 六套完整字体／113 项 | 未到达 | 6／6、113／113 |
| 原 Native 120 秒 | 未到达 | 无该门槛内就绪记录，严格判定 false |
| 后续 Worker 重启／Native | 未到达 | 重启检查通过，随后 Native ready=true |
| 正常注销、回选角、两浏览器关闭 | 通过 | 通过 |
| 严格完整验收 | **false** | **false** |

R1 进程 exit 1，原失败为 `FIRST_PLAYABLE_SCENE_NOT_OBSERVED`；正常清理中仍收到 LogOutSuccess。R2 进程 exit 0、功能摘要通过，但原分类器的共享 Native 任务窗和 Native 120 秒两项未通过。因此不能把后续 Worker 就绪、完整 HUD 截图或功能摘要倒填成首次就绪通过。[R1 游戏阶段](r1-game-summary.json) · [R1 原判定](r1-original-acceptance.json) · [R2 游戏阶段](r2-game-summary.json) · [R2 原判定](r2-original-acceptance.json) · [任务窗实际 owner](r2-quest-owners.json) · [完整字体 SHA](r2-original-fonts.json)

R2 运行包正文在首帧后 82,230ms 完成；首步同期 KeepAlive 为 4,761ms，前后基线约 133ms。Gateway 只观察到一次成功移动准入，累计处理 578μs，不能据此定位其余延迟发生在浏览器、传输、网关排队还是其他环节。R1 的完整静态地图正文与权威出生点已收到，仍未出现可操作场景；运行包没有完成正文的记录。这些现象不构成唯一根因。[逐阶段时间及移动](r2-game-summary.json) · [R1 阶段](r1-game-summary.json)

新观察器只订阅已经启用的 CDP Network 事件，分别记录请求、响应头、正文片段、完成和取消；不发送协议命令，不保存 URL、会话／意图 ID、请求头、正文、凭证或签名。另一个既有观察器只保留 JSON 状态枚举／证明存在布尔值和页面可见性。两轮桌面均全程观察为 visible／focused，轮询均观察为 no-store／Vercel MISS／CF DYNAMIC；这些只是本轮对照的事实。CDP 的完成事件时钟有毫秒级先于头／数据通知的情况，不据此推导负下载时间。[R1 批准关联](r1-auth-analysis.json) · [R2 批准关联](r2-auth-analysis.json) · [R1 桌面](r1-desktop-wire.json) · [R1 手机尺寸端](r1-mobile-wire.json) · [R2 桌面](r2-desktop-wire.json) · [R2 手机尺寸端](r2-mobile-wire.json)

新增观察器 8 项检查、既有正文／隐私观察器 2 项检查通过，合计 10 个不同用例、0 跳过。复测命令为在 `apps/web` 运行 `node --test scripts/test-auth-wire-observer.mjs`。本轮没有应用改动，未重跑无关构建或后端全集。[新增 8 项](auth-wire-tests.log) · [相邻 2 项](adjacent-observer-tests.log)

两张游戏截图已逐张查看：首次繁中任务窗仍是备用布局并显示资源准备 21／113；后续 Worker 重启截图已出现 Native HUD。截图仍有英文欢迎文案、人名等混合语言，未计为 Windows 像素或十语 1:1 通过。[首次任务窗](r2-first-quest-fallback.png) · [后续 HUD](r2-hud-after-worker-restart.png)

33 份原追踪 WIP 的原始 SHA 全部保持，无私有源码覆盖或活跃自有进程。两轮各通过普通协议创建一个专用 QA 角色并正常注销，角色保留；本轮没有 SSH、还原数据库或重读全部旧行，不能沿用上轮的总行数宣称新的全库保全。查询原 f05 部署的历史运行日志未取得记录，也不能证明请求没有到达服务。[保全与范围](preservation-and-scope.json) · [历史日志查询边界](historical-runtime-log-scope.json)

下一项保留 f05 原失败，捕获真正失败的批准交接时序后再定因；同时检查首次 Native 准入与首步传输延迟。两次 e0 登录成功不能宣称 f05 已修好。完整 Windows R16 → Web 100% Candidate Goal 继续 active，三职业完整路线、语言与像素、战斗／组队／交易／阿洛、其他地图与 Native 同档及最终人类 Accepted 仍按原范围推进。
