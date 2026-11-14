# R16 任务窗关窗首步与运行包重试

本轮基于 `1cc89f69a05dd35046f4497254a14b91128da1ed`，候选 `1cc89f69a05d-native-input-54d41c20`，生产构建 `ZNY8y6QXDORf2V8-x1MPB`。保留原有未提交改动。**本候选尚未发布，100% 1:1 Candidate 未完成。**本轮公网读回仍为 `888dd59046fa625989c8d9df903f5fff0a89035d`；Worker 正文 30,521 字节，SHA256 `18920fbeaa0eeb7966400e84a9cf16bc9ddba4fbb46d74128425557ea11697b5`。

## 修复

- 运行包下载在 App 启动前失败时，Page 撤销不可用的运行时，保留 URL 对共用任务窗、背包和 HUD 的请求。此前清空请求后，同一后端重试不会重新运行画布选择 effect，导致请求一直关闭。显式关闭选项、失败后的人工重进和 App 只启动一次的规则保留。
- 任务窗已在 Native 关闭时，移动键读取当前运行时、角色/世界、布局、语言和新鲜帧的输入占用状态。没有有效读数时保留原 React 边界；NPC 对话、商店、仓库、输入框、快捷键录制和其他窗口继续拦截。
- 移动键订阅调用最近一次已提交的处理函数。Q 状态及普通重绘不再移除、重装同一个物理按键尚未到达的监听器。按住方向、100 毫秒轮询、Shift、组合方向、释放和失焦清理维持原行为；被窗口取消的按住不会在关窗后补发。

## 检查与真实浏览器

候选原源码通过 **103 项定向检查 + 18 项快捷键相邻检查**、Next 生产构建和 TypeScript。先保留了实际源码红灯：UI 请求 3 项、直接按键 2 项、同次事件中较早监听器触发界面提交 1 项。旧测试夹具另缺少现有退出对话框 ref，产生的 6 项基线失败和补齐夹具后的结果均保留。

| 用例 | 结果与范围 |
| --- | --- |
| 最终候选默认 WebGPU，首次 WASM 故意返回 503 | App 未启动且允许重试；同一文档正常注销、选回同角色、重进后 HUD/任务窗恢复。当前会话 113/113，约 56.3 秒 Native 就绪；8 次 Q 开→关后立即单按方向均有服务器回包并到达。彻底关闭 Chrome 后，同角色重登约 1.3 秒 Native 就绪，反向走一步及两次正常注销存档通过。 |
| 最终候选共享 WebGL2 | 当前会话及浏览器重开后各 113/113，约 52.1 秒 / 1.3 秒 Native 就绪；同样 8 次关窗立即移动、反向移动和存档通过。 |
| 最终候选真实 CDN 冷启动 | 未通过。两次 WASM 读取均在原 120 秒期限中止，分别收到 28,936,998 / 24,870,936 字节，未达到完整 31,441,753 字节；正常重进及额外 180 秒观察仍未就绪。响应头均已收到，不能归因为没有出响应头。所有自建进程已关闭、原档摘要不变；该失败不计为完整冷启动通过。 |

前两行是**行为验证**：以本次固定发行版本的真实 JS/WASM 和字体字节供给浏览器，逐文件核对摘要；没有伪造世界快照、移动回包或缓存内容。运行包由明确标识的 Playwright 响应供给，新字体请求通过独立本机 HTTP 镜像和真实 Worker 读取。它们不代替公网冷下载验收。原来的 120 秒启动、113 项完整预取、首次单按到达及正常存档门槛没有放宽。

测试创建的是普通 1 级角色；原三十级战士档的 16 类个人数据按原验收器比对保留。这里不宣称完整三职业新手至 30 级路线已重跑，也不代表其他窗口、动态战斗、Hosted、纯 GPU 实体及 Windows 窗口均已完成。画面中的既有 NPC 名称/译文问题仍按语言像素差距继续验收。

## 保留失败

- 原版、以及中间候选均保留过“Native Q 已关闭，第一次 Arrow 没进入 Shell、没有网络移动”的真实事件追踪；中间候选另一次 8 轮通过没有覆盖掉失败。新测试进一步复现较早捕获监听器提交重绘，导致浏览器跳过已移除的后续监听器。
- 中间候选真实 CDN 首轮在 120 秒内未启动。第一次 JS 响应头约 95 秒，WASM 在原期限中止；正常重进后完整 WASM 收到，但简中字库仍在读取、HUD 未在额外 180 秒观察内就绪。此记录不计通过，也不能由此把字体等待误写成 UI 请求仍被清空。
- 最终候选第一次受控失败后的回执为 false：旧验收器选中了已正常退出会话的 cancelled 预取，而重进会话实际上是 113/113、HUD/任务窗已就绪。失败保留；改为选当前会话最近的 HUD 预取，并明确要求 113 项，随后重新完整实测。先前取消的会话仍留在回执中。

## 复验与保全

从仓库 `mir2-web3/apps/web` 运行本次相关现有测试：

```powershell
node --test scripts/test-r16-keyboard-ownership.mjs scripts/test-bevy-quest-ui.mjs scripts/test-bevy-quest-locale.mjs scripts/test-bevy-runtime-policy.mjs scripts/test-bevy-runtime-package-selection.mjs scripts/test-bevy-hud-ui.mjs scripts/test-bevy-bag-ui.mjs
node --test scripts/test-use-window-keydown.mjs scripts/test-r16-quest-close-shortcut.mjs scripts/test-r16-belt-shortcut-diagnostics.mjs
```

实际浏览器脚本和私有原始回执保存在 `C:/r16-native-input-recovery-20261112/game-r1/`，入口及摘要见回执的 `scripts` 字段；它们需要本机隔离 File 夹具及匹配构建。用例 `quest-after-r7` 为默认后端受控首次失败，`quest-after-r8` 为共享 WebGL2；两者均使用普通登录、选角、Q、物理按键和正常注销。验收不要求玩家执行这些开发命令。

原 62 个文件逐字节保持原摘要。Shell 内原动画绘制差异完整保留，构建和提交只取本次键盘补丁；另外两个绘制测试文件未改、未纳入。运行包、三份小型 WASM、27 个生成输入和原存档摘要均核对；没有部署 Gateway、修改 DNS、官网、生产存档或钱包资产。

[完整分项回执](receipt.json) · [103 项检查](candidate-adjacent-green.log) · [18 项快捷键检查](shortcut-adjacent-green.log) · [构建与类型检查](production-build-r2.log) · [默认后端当前画面](quest-after-r7-hud.png) · [共享 WebGL2 当前画面](quest-after-r8-hud.png)
