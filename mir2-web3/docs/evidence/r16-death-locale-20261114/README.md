# 死亡与回城复活文案十语

2026-11-14，在 R16 共同代码线上补齐 `ui.death.title/message/revive/reviving`。四个现有 Page 调用原来在九种非英语界面都退回英文；现在四段文案在十种语言都有正式登记，进入玩法词库加载前后保持一致。未更改死亡判定、复活请求或服务器回包处理。

既有浏览器词库完整性检查登记这四个实际使用的 key：修前 17/28 通过、11 项失败，修后连同帮助及任务语言检查共 33 项通过、零 skip。日志展示副本只规范行尾空格和LF换行，原始字节与 SHA 单列。[修前检查](before-tests.txt) · [修后检查](after-tests.txt) · [日志来源](log-provenance.json)

组件观察器直接从当前 `page.tsx` 提取真实 `DeathReviveOverlay` 函数和四个译文属性表达式，使用真实 `buildTranslator` 与 CSS 渲染。十语 × 360/1024 像素 × 正常/请求中共 40 个浏览器画面，文字、禁用态和边界通过。原组件未改；九种非英语的 36 段文字从全部英文变为各自语言，零 page error，浏览器及临时 HTTP 服务已关闭。[修前原回执](before.json) · [修后原回执及全部截图索引](after.json) · [观察器](component-observer.mjs)

root 已查看简繁、葡、俄、印地、印尼、越、泰、阿、英语代表截图；未见文案截断。窄屏长句会按原组件自动换行。[修前简中](before-zh-CN.png) · [修后简中](after/360-zh-CN-ready.png) · [印地请求中](after/360-hi-busy.png) · [阿语](after/360-ar-ready.png) · [审图与 SHA](visual-review.json)

边界：这是语言与实际组件显示的本地 Candidate；未用此组件快照替代真实死亡／正常复活流程，也未证明它的自有弹窗外观与 Windows R16 或 Crystal 提示框 1:1。已提交 Windows 的 V 键只在 HP≤0、maxHP>0 时发回城复活，Crystal 原提示框见 `GameScene`／`DiedTip`；其余输入与外观仍需对照。本批随b19尝试发布，因公网字体504和共享HUD失败已回退，正式恢复 `e6c923b7c6`，本批未留正式；原失败与回退出码均保留。[发布回执](../r16-name-autosize-20261114/production/README.md)。全局 Goal active。
