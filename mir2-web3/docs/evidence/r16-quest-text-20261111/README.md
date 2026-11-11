# R16 Web 备用任务窗目标与章节文字

基于已提交的 a852afd202，在普通 /game 入口验证默认 React 任务窗。初次验收时线上为 ef0d1ee9dc，本记录覆盖当时的本地生产候选。现已正式发布 78c900ca36，见[公网发布与复验](../r16-quest-release-20261111/README.md)。

任务目标、返回 NPC 提示、章节名、章节进度和成长奖励说明使用 Windows 现有十语词库。击杀目标支持静态 Defeat 句子和 ChangeQuest 的 Kill 句子，怪物名称由同一词库提供。每行的 current、required、done、任务状态和奖励选择索引沿用服务器值。未知目标保持原始文本。

同一份原始 File 档分别进入旧版与候选。旧版可见 Report to Assistant Jane、Village Beginnings 和 Journey。候选显示向助理簡報告、村莊起步、當前主線，以及章節 1/6、本章 0/4。阅读与选择任务期间记录到零条移动或接交命令。 [旧版](before.png) · [候选](available.png)

候选用真实方向键逐格走到简助理附近，点击当前 NPC 实际提供的接取链接。服务器返回首套巡逻装备可提交及 1/1，窗口同步显示返回助理 Jane。实际交付后，CompleteQuest 确认首项完成，随后权威快照解锁装备任务。窗口显示本章 1/4、當前主線 1/26 和装武器 0/1。 [可提交](ready.png) · [后续任务](followup.png)

正常退出后，网关使用同一份私有服务配置与数据重新启动。普通登录读回首项已完成、第二项可接取、金币 100 和位置 (285,607)，再正常退出。原始账号档与原角色文件 SHA256 校验通过。 [分项回执](receipt.json)

生产构建和 56 项 Node 定向检查通过。22 条主线的静态、动态击杀及职业练习、4 个等级成长奖励和 6 个章节标签均覆盖十种语言。原有任务本地化和新手旅程脚本也通过。翻译能力测试的模块读取器已补齐 JSON 导入，以运行实际词库依赖。

复测命令在 mir2-web3/apps/web 目录运行。

```powershell
node --test scripts/test-r16-native-localization.mjs scripts/test-r16-language-parity.mjs scripts/test-bevy-quest-ui.mjs scripts/test-bevy-quest-locale.mjs
node scripts/test-newcomer-v2-journey.mjs
node scripts/test-quest-localization.mjs
```

共享 GPU 任务窗、实体手机与备用窗毕业目标选择仍需验收。此记录覆盖默认备用窗和一个实际任务接交流程；公网签名与任务窗另见发布回执。
