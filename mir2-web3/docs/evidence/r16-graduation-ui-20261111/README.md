# R16 毕业目标与共用任务窗

基于已提交的 13271ca27f，补齐备用任务窗毕业目标的十语显示。标题、职业装备、技能、条件、来源和 D605 地图名沿用 Native 词库及当前 Crystal 地图索引。挑战补齐蜈蚣、钳虫和补给提醒，装备与技能保留 Native 的目标说明。既有等级、26 项完成条件、任务状态和奖励索引不变。

57 项 Node 检查、22 条权威任务定义检查、新手旅程及原任务本地化脚本通过，生产构建与 TypeScript 通过。三职业 × 十语的目标内容与 Native 配置、地图和词库逐项比较。 [分项回执](receipt.json)

使用已有真实 30 级毕业档的隔离副本，经普通密码登录和选角进城，读取服务器实际提供的 26 项已完成任务。繁中依次选择装备、技能和挑战，没有发送世界动作。随后默认 WebGPU 共用 HUD 和任务窗就绪，按 Q 打开真实共用窗口。正常退出后，在新的英文浏览器会话再次登录同一角色，使用支持的备用 UI 开关复查三类目标，再正常退出。

| 职业 | 繁中技能 | 原金币 | 原经验 | 结果 |
| --- | --- | ---: | ---: | --- |
| 战士 | 野蠻衝撞 | 76,360 | 19,497 | [备用窗](warrior-fallback-skill.png) · [共用窗](warrior-shared-quest.png) |
| 法师 | 地獄雷光 | 75,960 | 16,221 | [备用窗](wizard-fallback-skill.png) · [共用窗](wizard-shared-quest.png) |
| 道士 | 淨化術 | 53,295 | 18,263 | [备用窗](taoist-fallback-skill.png) · [共用窗](taoist-shared-quest.png) |

六次正常退出均有回包。每个角色的 26 项任务与位置、朝向、金币、经验、物品、装备、技能等 16 项存档字段保持一致。三个原始账号文件的 SHA256 不变，所有自建浏览器和服务已关闭。 [挑战繁中](taoist-challenge-zh-TW.png) · [新会话英文](taoist-challenge-en.png)

前置冷探针在约 59 秒后观察到共用 WebGPU 就绪，本轮三份毕业档的探针分别约 8、8、15 秒。文件下载完并不等于 UI 已接管，短观察窗也不能证明永久启动失败。这里记录的是隔离服务、真实资源读取和软件环境下的观察，没有据此宣称公网共用窗口已验收。

浏览器复测步骤：用正常 30 级且 V2 已完成角色登录，按 Q 打开任务日志，检查三类毕业目标；等待共用窗口接管后再次按 Q；正常退出，用新的英文浏览器会话同角登录，检查任务、物品和金币。复测 Node 命令在 apps/web 目录：

```powershell
node --test scripts/test-r16-native-localization.mjs scripts/test-r16-language-parity.mjs scripts/test-bevy-quest-ui.mjs scripts/test-bevy-quest-locale.mjs
node scripts/test-newcomer-v2-journey.mjs
node scripts/test-quest-localization.mjs
```

本轮是本地生产候选，公网仍为 78c900ca36。没有重跑当前前端的完整 1–30 级战斗任务线，没有重建共用运行包，也没有覆盖共享 WebGL2、其他语言的实际浏览器、实体手机或人类 Windows 窗口验收。毕业目标的再次点击取消、关闭重开与会话选择状态仍需对照 Native 完成。
