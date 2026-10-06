# 原版共享挖矿 Candidate — 2026-10-07

用户授权「来实现挖矿吧」。本轮基于 `3c255155b3b04f1c415913d5c8af024200bf7455`，
在隔离 classic 分支实现普通玩家采矿，不修改受保护 E 工作区、公开网关、更新
feed、D/F 玩家安装和真实存档。挖矿是经典 Goal P6 的一部分，升级武器另行验收。

## 普通玩家入口

- 三职业均可使用原版 `PickAxe`，等级要求 12，原价 2500 金币。
- 普通铁匠 `BuySell` 购买并装备到 Weapon；背包中仅有矿镐不能挖矿。
- 比奇矿洞入口 `0 (663/664,215)` 通往 `D401 (24/25,181)`，保留原版地图路线。
- 靠近矿区障碍墙格，按住左键朝墙挥镐。松手停止；右键、移动键、Esc、UI
  阻挡、失焦、死亡、换图和换武器都会取消旧意图。
- 原版矿物直接进入背包；背包满但腰带有空格时可落到腰带。全满时无矿物奖励，
  仍照原版消耗矿石次数和成功敲击耐久。有空格但超重仍可收矿。
- 支持普通铁匠出售矿物；卸下矿镐到背包后走普通 Repair 服务。

## 规则与权限

`generate-crystal-respawn-manifest.mjs` 的 `MIR2_CRYSTAL_MINING_ONLY=1/check`
仅生成/核验新的 `crystal_mining_manifest.json`，不覆盖其他生成数据。
原版 DB117/custom0 与 `Configs/Mines.ini` 导入 23 矿区、两个矿集；当前 platinum
profile 保留 208 运行地图/209 资源地图，启用其中 14 个原版矿区和八种矿物。
profile 版本仍为 27，依赖从 13 增至 14；新 contentHash 为
`4b95ee1a25df34185d94240613c36673a093a8ee27ca80509bbe0b7acb5beaf4`。
冻结旧 Candidate 的 profile hash 不可直接当作本轮包的证明。

共享 Zone 独占矿点、石数、补充时间、动作期限和随机结果。正常 `C.Attack(None)`
在没有有效近战目标时进入挖矿；其他技能不能借此挖矿。真实武器 UID、CanMine、
耐久、等级装备准入和服务器状态从认证个人 session 获取，客户端不能提交奖励、
纯度、命中率、矿点状态或调试命令。未知/越界碰撞、普通地面和非矿区均不产矿。

首次空矿点仅补充 0–79 次，之后每次消费一次。空矿点严格超过五分钟才补充。
命中率为源 HitRate 加武器准确 ×10；掉率为源 DropRate 加总 MineRate。源槽位
的间隙、包含端点和矿区最后覆盖优先均保留，不人为提高出矿率。成功敲击磨损
5–19 点，Strong 正值可减免且至少 1 点。矿物 CurrentDura 是纯度，通常 3000–
15000，源奖励可达到 24000；MaxDura 保持原始模板值，不能把两者混为一谈。

在现有共享单写锁内 prepare → 个人矿物/耐久一起 CAS 保存 → sealed Zone commit，
避免移动、受击或离开在中间改变已接受的挥镐。保存前同步最新共享生命和位置/
朝向；矿镐挖坏立即刷新共享属性及生命上限。保存成功前不向客户端展示得矿。
确定的保存前失败回滚个人资产且不消费共享矿点。文件已 rename 但目录同步失败
属于提交结果不确定，磁盘可能已保存；沿用写栅栏阻止重试，恢复后保留可能已提交
资产，不能声称每一种保存失败都没有写盘。

Zone checkpoint 升至 v7，将矿点/期限/序列/待触发效果纳入认证根。v1–v6 仍按原根
验证；v6 已认证掉落 custody 保留，未绑定的注入矿点忽略。共享完整 checkpoint
恢复与增量主备重放分开：后者当前使用重放机器的时间，随机挖矿不保证一致，
本轮仅验收单 Gateway 和完整 checkpoint，不批准 HA 增量重放。

## Windows 表现

本地只保存当前挥镐手势，不堆积将来的攻击；发送失败丢弃旧攻击。主人确认的
匹配动作转换到原版 Mine/Attack2，旁观者仍接受原版 ObjectAttack(None) 的普通
Attack1。只允许当前地图、位置、朝向、序列和期限内的确认触发 Mine。
服务端发 400ms 延迟 MapEffect12 和共享 Rubble。原版音效 10091 尚未接通。

## 验证和交付状态

实际测试、原始失败记录和四张离屏 GPU 图片见
[本轮证据](generated/player-qa/mining-20261007/README.md)。
离屏 fixture 使用真实 D401 地形、生产 packet/pose/atlas 几何和生产背包 UI；
`runtimeWorldRenderer=false`、`liveAcceptance=false`，不能算实际服务器鼠标验收。
测试中的角色等级/金币/位置和部分分支时间是明确的可信测试准备，不算自然练级
或持续挖矿时长证明；原版掉率和时钟未降低。

源码、有界回归和真实D401严格checkpoint恢复已完成；默认生产网关库编译通过。
独立 Linux [CI 37510943095](https://github.com/Zombieliu/mir2/actions/runs/37510943095)
对 exact source `e397fdc90404b13e827aa4e5dc89078421d9851a` 全部通过：game-data
61+5+3、simulation 9+5、Gateway 挖矿/真实 WebSocket 9、完整 live authority
拒绝恢复 2。原始日志和 source-bound receipt 保存在本轮证据目录；该 Linux
执行不代替 Windows 实机鼠标和配套发布验收。
配套候选打包、公开发布、受影响 Windows 实机鼠标按住
采矿和断线验收尚未执行；不能把现有 R17/feed14 算作新增挖矿已经上线。P6 的
Windows 精炼、检查、领取和完整 P1–P8 Goal 继续开放。
