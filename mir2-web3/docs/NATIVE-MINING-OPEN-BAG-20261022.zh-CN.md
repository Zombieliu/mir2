# 开背包持续挖矿修复（2026-10-22）

人工验收反馈：玩家需要在挖矿过程中打开背包，直接观察新矿石及纯度。旧版本把普通背包当成全局采矿阻断，按 I/F9、打开背包及后续动作转发都可能清除 AutoHit，因而无法边挖边查看产出。

本次按用户要求调整普通背包行为：开始挖矿后开关背包、悬停查看 tooltip 保留原挖矿动作；背包打开时，点击未被背包遮挡的相邻矿壁仍可开始挖矿。背包按下与世界按下分别处理，背包物品、关闭按钮等不得透传为世界移动或攻击。按键依据实际绑定识别。

移动、失焦、死亡、地图/角色来源/工具变化以及模态操作仍取消动作。物品使用、丢弃、拆分等操作弹窗属于模态，不等同于普通背包悬停查看。本次不改变挖矿间隔、产矿概率、服务端校验或账号物品；30级仍约1030毫秒一次尝试。

## 验证记录

完整 Windows host 回归实际1166通过、0失败、7项既有专项忽略；包含6项新增背包输入/Bridge测试。命令为 `cargo +1.95.0 test --manifest-path apps/game-client/platform-windows/Cargo.toml --locked --offline --jobs 8 -- --test-threads=1`，指定既有真实 Native 资源根。执行前后572项被记录的本地客户端/包源码输入哈希一致。记录位于本机 `C:/mir2-build/mining-open-bag-20261022/builds/20261022T123000591837Z-test`。

首轮挖矿定向测试27通过、2失败、1忽略，失败保留在 `20261022T121747000446Z-test`。新增测试恰好推进1232ms，在第三帧经 Duration 转为浮点毫秒后产生 `3695.9999999999995 < 3696`；仅为新增UI行为测试增加1ms余量并补充上下文，原精准节奏测试和生产间隔不变。修正后的上述完整回归包含全部挖矿用例。

代码提交 `64769fc70401dc5a1926f9c6e6bab3739b45ca45` 已推送到 `codex/mining-open-bag-20261022`。随后真实 Native 构建 exit0、sourceDrift为空；产物131188224字节，SHA256为 `2a2179f3789145833f3fbcf69430ed84d66aa059e81eed46d32b7095c760ab36`。测试发生在提交前的修改工作区，回执保留原HEAD `62312e4766`，不将该HEAD误记为测试源码只有旧版；构建回执绑定新提交。实际运行的是本机 `C:/mir2-build/mining-open-bag-20261022/client/mir2-platform-windows.exe`，桌面“01 挖矿制作钓鱼验收 - 30级”已指向新版启动器。

助手已在真实窗口用原普通登录进入30级战士 LeisureQA100；继续使用本机隔离 File 存档和原网关，没有写入QA物品。D401(38,178)装备矿镐，背包打开时点击上方矿壁启动；松开后持续挖矿，期间按 I 关闭再打开背包仍保留同一采矿手势。一次观察段包含至少90次连续尝试，间隔约1030ms。该客户端队列记录用于证明输入持续，不单凭它判断服务端成功产矿。

本轮自然新增两块黑铁矿石，纯度分别12和10；背包空格33→32→31，金币保持805160。第二块在背包持续打开期间直接出现，未靠重开背包刷新。普通向右走一格停止挖矿，到D401(39,178)后悬停核对两块新矿石。随后通过 Alt+X／Yes 返回选角，再正常 StartGame，两块矿石与金币均保留；这是同一客户端和网关内的正常保存重进，不是冷进程或服务器重启验证。重进后再次开包启动挖矿，悬停纯度12矿石时仍持续挥镐，日志中同一手势继续发送。

截图依次记录[挖矿前背包](generated/player-qa/mining-open-bag-20261022/01-bag-before.png)、[开关背包后第一块新矿](generated/player-qa/mining-open-bag-20261022/02-new-ore-after-bag-toggle.png)、[持续开包出现第二块新矿](generated/player-qa/mining-open-bag-20261022/03-second-ore-with-bag-continuously-open.png)、[纯度12](generated/player-qa/mining-open-bag-20261022/04-black-iron-purity-12.png)、[纯度10](generated/player-qa/mining-open-bag-20261022/05-black-iron-purity-10.png)、[保存重进](generated/player-qa/mining-open-bag-20261022/06-ores-preserved-after-save-reentry.png)、[悬停时继续挖矿](generated/player-qa/mining-open-bag-20261022/07-tooltip-while-mining.png)。[结构化记录](generated/player-qa/mining-open-bag-20261022/outcomes.json)包含截图哈希和构建、测试来源。

## 人工复验

1. 装备矿镐，靠近真实可挖矿壁，按当前规则转向后再次点击启动。
2. 松开左键，按背包绑定键打开背包，保持游戏前台；连续观察至少十秒，挥镐应继续。
3. 悬停已有矿石查看提示、开关背包，仍应保持原矿壁方向；记录自然出现的新矿石，不能仅凭既有数量推断新产出。
4. 背包保持打开，停止后点击外部相邻矿壁重新启动；点击背包表面不能导致地图移动或攻击。
5. 普通移动、卸下矿镐或进入物品操作弹窗后停止；正常退出并重登检查已获得矿石保存。

这份记录只覆盖本次 Native 交互修复。公网部署、全项目 Candidate100 和最终人类 Accepted 均不据此宣称完成。
