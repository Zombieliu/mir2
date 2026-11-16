# R16 Worker缓存修复：已正式发布

2026-11-13，正式入口 [numeron.obelisk.build/game](https://numeron.obelisk.build/game) 已切换到 `f6acaad6b0046553bcc5d2fb15784b29877f5eb9`，部署 `dpl_FmUSMsypVxYjrBHfAEHVaPZovHrC`。**发布操作通过，完整公网试玩仍失败。**

- [Stage43265707180](https://github.com/Zombieliu/mir2/actions/runs/43265707180) 与 [Promote43268702342](https://github.com/Zombieliu/mir2/actions/runs/43268702342) 均成功，测试后切换同一构建，target与域名alias均指向新版，未回滚。
- 替换a4f99c2111；辅助程序`059466951b7663e98d020eb1ac529dc87a3d6b2f`。55份输入与已提交源码绑定，147项/构建和68项发布保护通过；9条公网路径完整正文及摘要复核通过，包括Worker和6个固定小型客户端文件。[源码绑定](committed-source-binding.json) · [公网文件](final-public-readback.json)

## 已通过

简体、繁体普通Chrome出码、待确认与取消通过。手机尺寸官方钱包PWA用独立Testnet虚拟Passkey完成一次登录签名；普通新角色进比奇、实际一步、繁中备用Q、HUD预取113/113与正常注销通过。首个可操作画面在点击进游戏后7.314秒，真实walk→UserLocation为2.421秒，未修改原8秒门槛。HUD最初107/113，原6项失败记录保留，补读后113/113。

公网Worker在测试浏览器内真实stopped→running，同一原版Prguse/1.png返回200、66374字节、SHA256 `4efed8c95f0449f2ecbd2d5623f6aed0d2d6993ad7873cc41b82a59e737134fa`；CacheStorage字节及完整配置/分区计数前后相同，响应确认fromServiceWorker。测试没有发送CONFIG。该分项实际通过；公网未声称零新增上游请求，只有[本地受控测试](../README.md)实测零。

## 保留的真实失败

共享HUD在停Worker之前就未就绪。运行包已交付并启动，但最终quest错误为 `quest UI font or skin failed to load`，hud/quest ready均为false。新增检查同时要求“缓存恢复且Native保持ready”，因此其passed=false，整段签名试玩也为false；**没有重写原回执或把分项成功当成全通过**。[完整分项与原回执摘要](receipt.json) · [重启检查原失败](worker-restart-partial.json)

本次任务窗截图为备用React画面，人工查看也仍有既有NPC名字碎片和夹杂英文，不算共享Native或完整语言通过：[实际截图](public-fallback-quest.png)。受控默认WebGPU/共享WebGL2全流程通过不覆盖公网字体/皮肤失败。继续字体来源/响应头长尾、49秒来源链和30秒页面预算、120秒Native冷启。

此前首步19.7秒与独立a4基线18.635秒仍失败；本次2.421秒只是一次观测，不能归因为缓存修复或据此关闭通信长尾。被动帧统计只存类型/长度/时间，无包内容；大小为解码载荷，不是网络压缩流量。

## 取证与权限边界

执行前冻结17份QA脚本并校验摘要，实际手机侧为设备模拟，实体摄像头仍待测。仅一次登录批准、一次普通QA建角和普通退出，钱包资金操作0、所有自建浏览器关闭。认证原文只留本机，不提交仓库。QA时间检查修正了本机与服务器约225ms时差的误判：严格使用响应中120000ms真实request/intent TTL与产品已有30秒clock skew；服务端仍验证MAC，测试未自称验证MAC，所有URL/同一会话/登录意图边界保留。此前两次在钱包批准前的QA时间检查失败也保持原记录。

65份原未提交文件保持逐字节一致。此次未部署Gateway（只读health观测为7033a012bf）、DNS、官网或链上合约，未迁移玩家存档。完整三职业1–30当前路线、全部地图/技能、阿洛、Hosted、Windows窗口、实体手机和最终人类Accepted仍独立开放。

- [Stage回执](stage.json) · [Promote回执](promote.json) · [发布保护68项](release-tests.log) · [收尾时公网版本/Worker再次读回](final-check.json)
- [本地147项/两后端/完整浏览器重开](../README.md) · [前次正式站分项与首步失败](../../r16-hud-recovery-20261112/production/README.md)
