# R16 行走包有限恢复候选

2026-11-14，产品提交 `91b8fc9b4f5170cb37b6619b991707ed76a34d73`，发布保护提交 `3588207432`。本轮继续已发布的[任务目标、方向与移动确认修复](../r16-route-direction-20261113/README.md)，补齐怪物行走包部分正文失败后的恢复。**候选已 Stage/READY，尚未 Promote；实际行走帧与完整本地流程仍未通过，不能记为已部署或已完成。**

当前正式站保持 `f6603c89a3`／`dpl_3eNPP4uaj3nT3HsAMhJUUoXpN4Dh`；候选为 `dpl_44ctBTrdq2T5fb91EjrUEt6B3GUk`。本页的 Stage 回执仅证明候选就绪；后续发布仍需本地实际动画资格和独立公网复验。

## 产品边界

原行走 JSON 的两次读取及 8 秒正文 idle、45 秒绝对期限不变。仅在可重试的传输故障耗尽后，接入既有一次 20 秒 pinned compiled 读取；401/403/404、Retry-After 和完整但短缺／损坏的正文不授权此恢复。没有增加 JSON 请求、改变怪物 AI、任务、存档、Gateway、官网、DNS 或素材／运行包版本。

编译载荷还原同一份 764,724 字节 JSON，SHA256 `41fcaae2757ad0351da2e3840bf54008952b5c2714441530bb64c21f5c9fd64f`，240 张原始 PNG；Monster/139 按实际 Walking skip3 校验。Standing 与 Walking 图片集合不相交，站立图通过不能替代行走消费。完整 JS 字节及原始载荷见[编译包固定值](walking-chunk.json)。

## 已完成的限定检查

- 以提交版 Shell 为基础的 118/118 检查通过，零 fail/skip；70/70 发布保护通过。另有独立离线观察器 19/19，属于 Node 模型验证，不能代替真实浏览器动画。[检查](checks.json) · [观察器原回执](observer-offline-tests.json)
- 生产类型检查与构建成功，56.961 秒，BUILD_ID `Lgy7EhuAT0ZovaqfJlwVL`。974 份受控 Web 代码／配置闭包核对通过；只有具名 CRLF 归一化与既有固定运行包清单变换，原 Shell、Native 和容量 WIP 没有混入。[构建](build-receipt.json) · [源码闭包](source-closure.json) · [提交绑定](candidate-qualification.json)
- GitHub Stage [43472618560](https://github.com/Zombieliu/mir2/actions/runs/43472618560) 成功，独立 Vercel 读回 READY、正确项目／分支／来源。[Stage 回执](stage-readback.json)

本地准备的旧 `source-pin.json` 误继承了历史候选和四会话计数字段。其原文件及 SHA 保留，CI 的 source/base/changedFiles/baseline 没有错误；[显式资格清单](source-pin-qualified.json) 已以本轮构建和检查替换这些元数据，明确 qualifiedLocalSessions=0、actualAnimation=false。本页不采用旧字段作为验收依据。

## 保留的实际失败与下一验证

[五轮失败摘要](local-failures.json) 绑定原始文件 SHA，旧回执未覆盖。各轮均观察到两份 JSON 部分正文退役后，完整 compiled 包通过字节、SHA 和全部 240 PNG 校验；实际 Walking 帧仍为零，整体验收 false。

早期 GPU 接管的猜测已被后续 DOM 渲染诊断替代。r3 没有成功点中鹿；r4 点中并收到正伤害，但短按方向键没有接受位移。r5 的真实单击只攻击一次，鹿扣血后存活，持续按键 614 毫秒，在 579 毫秒收到 `(286,612)→(286,613)` 的权威位置确认并松键；没有怪物 ObjectWalk/Run/Turn 或快照位置差分，故两帧门槛仍失败。

该鹿与玩家都在导入地图的村庄安全区 `(288,616)`、半径10内。当前 Rust passive target 会排除安全区内玩家，现有场景不足以判断行走载荷是否被动画消费。接下来先在隔离副本以普通协议确认安全区外真实移动，再用普通界面走出安全区、单击攻击并撤开；全程保持正常碰撞、权威回包、单次攻击、存活、原 8 秒逐步进展和 120 秒 Native 门槛。不得注入移动、重置怪物或修改 AI 以通过测试。

暖场另有潜在测试顺序问题：旧脚本先刺激怪物、后等 compiled 完整读取；r5 没有执行到暖场，因此这是源码检查发现的竞态，不能记成已发生的失败。新草稿应先完成原有限传输观察再进行正常刺激，保留冷暖全部其他门槛。

## 冷加载仍开放

对上一轮正式 f660 公网成功功能回放的[只读时间轴分析](prior-cold-critical-path.json)：点击至首帧 30.667 秒，首帧至运行包正文 107.553 秒，共享 HUD 观测在首帧后 204.320 秒，原 120 秒门槛 false。运行包完成至 HUD 又间隔 96.767 秒，期间 Native 字体批次有部分正文失败和换源。

更早的 CSS 字体尝试确实存在，不能写成所有字体首次请求都在运行包之后。`ERR_HTTP2_PROTOCOL_ERROR` 是观察到的失败，不是根因结论；现有 Worker 字体观察没有逐次正文首末字节间隔。后续共享预取与运行包重叠仍需带 controller／配置／namespace 的有效条件及独立测量，本轮尚未实现或宣称性能改善。

整体 Windows R16 1:1 Goal 保持 active。实体手机、玩家原浏览器手势、公开旧档、完整三职业路线及其他地图／技能仍沿各自记录继续；这些检查不构成最终人类 Accepted。
