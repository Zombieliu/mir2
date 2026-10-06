# 经典三职业玩法补齐 Goal — 2026-10-04

## 2026-10-06 用户确认恢复执行：先完成可玩交付

用户再次确认按下表八项顺序快速落地。资源包直读改为后续优化，当前继续使用
已发布更新器 sequence 14；游戏公开 feed 仍为 R17/source6032。不把更新器发布
算作 R18 地图、经验或行会代码已经上线。保留旧 Goal，不能将未完成目标标为
完成以绕过 Goal 工具的覆盖限制；系统状态仍为 blocked，本轮实际工作已按用户
指令恢复，重新计算阻塞审计，不沿用之前的阻塞轮次。

本轮先收尾 P1，再依次推进 P2/P3；P4–P7 的功能实现排在这些交付之后，P8 最后。
固定 R18/source321316 的历史包与后续 P3/P5 源码分别记录，禁止混用源码、ELF、
EXE、资源目录和测试回执。当前独立分支以 47471920e5 为基线，不修改 E 盘主工作区。

快速执行采用有界验证：先解释上一轮最多四个合格攻击者的原因，再选择可形成
真实怪群的场景。既有通过结果在有效输入未变时保留；新增修复仅运行直接失败
场景及相关回归。测试编排失败、资格不足、真正玩法失败分别记录；不得降低
验收门槛、伪造攻击者、缩短原版时钟或把静态路线计划算成正常玩家成功。

| 当前优先级 | 下一项可审阅交付 | 完成依据 |
| --- | --- | --- |
| P1 | 怪群编排原因、移动/攻击失败复现与最小修复，随后配套候选版 | 正常输入与真实受击 trace、Windows 输入/画面、保存恢复；代码/构建/发布各自核验 |
| P2 | 新地图资源及普通往返、条件入口、Boss与装备/技能书获取 | 208运行/209资源闭包加实际玩家路线和获得物品，不用76跳计划代替执行 |
| P3 | 组队/Boss/PK、跨区身份/死亡边界及经验掉落的保存恢复 | 共享权威、真实奖励/拾取、重登/故障边界；既有单人首击及60秒保护承接 |
| P4–P7 | 宝宝PK → 行会剩余管理/普通战争/杀怪EXP → 矿区及Windows精炼 → 原版事件 | 每项完成正常玩家入口、权威处理、回包/UI、保存恢复和配套交付 |
| P8 | 全服寄售、夫妻召回/婚戒、英雄及装备觉醒 | 经典阶段后推进，不提前计入完成 |

每项继续分别登记 source / tests / packaged / published / installed /
playerVerified / humanAccepted。七项经典阶段仍全部存在剩余工作，不编造整体
百分比。生产切换需核对在线状态并保留安全退出/回滚；不停止其他游戏，不替换
用户安装或存档。可独立完成的代码和验证不等待例行确认，完成变更提交并推送。

> 2026-10-05 root integrates the eleven-file native Guild rank-rename slice
> after P3/source513eb83, applying only two Gateway terminal hunks. Source7
> merges its actual nested rank, retires exact current editor context, and is
> preserved before canonical255; hidden strict5s timeout and unsent/stale cleanup
> are checked. Root83 client/4 wire/4 FIFO/3 terminal/9 P3 checks pass, overlapping
> retained worker checks; one existing GPU case stays ignored. Native clicks,
> self/AOI labels, no-nonce ABA, paired network/build/rollout and fullP5 stay open.
> Protected authority/native queue/schemas/locks and R18 artifacts are unchanged.
> [Scope](NATIVE-GUILD-RANK-RENAME-20261005.md),
> [root evidence](generated/player-qa/guild-rank-native-20261005/README.md).


> 2026-10-05 root integrates the bounded one-manager cross-map solo ownership
> and native ground protection slice, plus identity-matched local typed Dead
> at actual same-tick impact. Original Manager RED0/6 becomes89 passing checks;
> the additional genuine life RED2/1 becomes3 passing cases,92 distinct total.
> Root selects57 original then52 related checks, all pass with overlaps retained.
> Award source issuance, online transfer/leave epochs, strict60-second clocks,
> custody/TTL and cold-root validation are checked in prepared local fixtures.
> Cross-Zone same-batch life and object-ID admission collision remain OPEN;
> party/Boss/PK, process handoff, SQL, network/native/human/progression stay open.
> R18/source321316 artifacts and current services/saves are unchanged.
> [Scope](CROSS-MAP-DROP-LIFECYCLE-20261004.md),
> [root evidence](generated/player-qa/cross-map-drop-20261005/README.md).


> 2026-10-05 exact frozen R18/source321316 completes one isolated prepared
> ordinary-protocol run. Wizard/Taoist actual natural Death and valid dead Magic
> inputs retain absolute MP541/260, items and life/position; normal LogOut and
> different fresh Login/StartGame restore alive128/541 and239/260 at0(288,616).
> Quiet and alive-save/relogin pass. Root verifies93 frozen public files and
> actual closed windows; historical R17 MP failures remain. Warrior180s death
> is inconclusive; all10 dense cases lack seven qualified attackers (maximum4).
> Seven-monster/native/human/fullP1/progression/load/capacity gates stay false.
> Existing realms/Caddy/config/feed/counters are unchanged. R18 is not rolled
> out; actual file updater/rollback passed separately, CDN login is pending.
> [Exact public result and root review](generated/player-qa/r18-prepared-network-20261005/README.md).


> 2026-10-05原生下载实测约100 KB/s，R17仍逐文件安装；R2原生入口404。
> R17的11个资源合包/真实R16差分/CMS与36对象闭包已准备；本机Cloudflare
> 尚未登录，未stage/promote或做CDN速度验收。实际R17→R18文件更新、删除、
> 内容缓存、个人文件与完整回滚通过；这是无加速器/无启动的隔离检查。冻结R18
> 与实际公网R17分别记录，F/玩家存档/游戏网关保持原状。
> [诊断与边界](NATIVE-DOWNLOAD-ACCELERATION-20261005.md)。

> 2026-10-05 root integrates the six-file shared Guild rank-rename authority
> Candidate at321316: actual unchanged-source RED2/6 becomes GREEN8/0; direct5
> and adjacent8 pass,21 unique worker checks. Root independently rechecks5+1
> overlapping checks. Durable permissions/current membership, source UTF-16
> names, revision+1 per valid request and member-only cross-Zone status7 pass.
> Native actor terminal, partial-roster merge/pending, AOI rank labels, SQL,
> ordinary war and other management remain open. Frozen R18/source321316 paired
> builds/Candidate/Bootstrap pass; actual updater/dense gates and rollout are
> still pending. R17 stays live; F and real saves are unchanged.
> [Rank scope](SHARED-GUILD-RANK-RENAME-20261004.md),
> [root evidence](generated/player-qa/guild-rank-20261004/README.md).

> 2026-10-04 source57729 paired CI/native builds pass, but clean actor preparation
> identifies exactly225 absent Monster049 generated files (224 frames + metadata).
> Root admits only that source-bound generated library; the action catalog remains
> tracked/exact and Candidate closure is not relaxed. Failed cache preflights are
> retained, no actors were copied by them. New exact-revision builds/package and
> isolated dense/dead acceptance remain pending; live R17/F are unchanged.
> [Packaging input scope](R18-ACTOR-CACHE-PREFLIGHT-20261004.md).

> 2026-10-04 root verifies frozen v6 pure attribution88/88 and31 inputs;
> victim-only damage and dead-level-up HP refill remain inconclusive alone,
> while actual R17 caster MP failures remain failures. R18 first pair/source51e8
> compiles, but overall CI37198506131 fails an unchanged R17 siege fixture after
> real bootstrap advances beyond today's18:00. Local real TCP/WebSocket RED0/1 turns
> GREEN1/1 when only that fixture chooses the next calendar day; source calendar
> and stale guard are unchanged. First failed logs/build attestation are retained.
> A clean final paired build/CI and native/ordinary dense gates remain pending.
> [Clock scope](SABUK-TRANSPORT-CLOCK-20261004.md),
> [v6 review](generated/player-qa/crowded-combat-escape-20261003/dense-v6-offline-20261004/README.md).

> 2026-10-04 root integrates the frozen18-file transient death/EXP slice on
> e0877a198: original meaningful RED Gateway1/5 and personal1/2 remain;
> final worker focused15 + adjacent61 pass. Root independently rechecks the
> integrated Gateway12/personal3, overlapping those15. Online Dead survives
> level-up HP/MP refill and same-epoch rollback; typed server-only life mirrors,
> pre-spend native rejection, explicit revive and post-award teardown pools
> are verified. Genuine HP0 login refills after RefreshStats; no saved Dead
> schema is added. Exact published R17 MP failures remain. New paired build,
> TCP/WSS/native/SQL/socket-resume acceptance are still open; F stays untouched.
> [Scope](DEAD-EXPERIENCE-CHAIN-20261004.md),
> [raw root review](generated/player-qa/crowded-combat-escape-20261003/dead-experience-chain-20261004/README.md).

用户已确认按重要性排序，并明确要求作为 Goal 开始推进。本目标没有另行指定
时间或 token 预算。之前的容量目标保持暂停；本目标不宣称 50/100 人容量已验收。

## 当前基线与完成标准

- 已发布：游戏 R17；当前公开更新序号 14 更新的是安装引擎，客户端与 Gateway 源码
  `6032ef8b3e27dd97bad0b20c8676ef9f185db64b`；公网完整文件与安装器哈希已核验。
- 当前修复分支：`codex/crowded-combat-control-20261003`，前轮源码与证据已推送；
  本轮集成 v27 地图和同图经验归属，代码、发布、普通协议与真人验收继续分开。
- R17 保持 32 张地图的原生图像验证范围，并携带 1,620 个原版地图布局文件。
  新源码准入 208 张 runtime 地图，另 1 张 D71653 仅携带原版资源，原生范围共 209 张；
  67,993 项可绘制引用缺失为零，Monster049 的 224 帧与源像素/动作相符，R18 已打包，尚未发布。
  164 图前轮证据保留；本轮见
  [资源与消费者编译](generated/player-qa/classic-map-closure-20261004/phase2-208-209/README.md)。
- 普通怪同图首击归属、5 秒边界、宠物/毒/TurnUndead 和 v5 存档已实现；
  跨图、地面 60 秒、原版组队/Boss 与死亡角色升级仍待接通和端到端验证。
  [范围与失败记录](CLASSIC-EXPERIENCE-OWNERSHIP-20261004.md)。
- 战士 0–30 有用户验收；法师/道士独立正常协议流程已有 52/52 节点及保存证据。
  功能完成、自然耗时、真人手感和三职业 1–50 完整验收仍分别记录。
- 每阶段采用正常玩家入口 → 共享权威处理 → 回包/UI → 保存/恢复的验收链。
  导入模板、预置金币/任务进度或单元测试数量不能替代实际玩家流程。
- 自动化验证只支持 Candidate；真人原版视觉/手感未验收时不能标为 Accepted。
  独立工作继续推进，不因等待一项真人反馈停止后续可完成工作。

## 顺序与阶段 Gate

| 顺序 | 工作 | Candidate Gate | 当前状态 |
| --- | --- | --- | --- |
| 1 | 战斗与移动稳定性 | 发布配套客户端/Gateway；正常三职业协议检查多怪受击、右键逃跑、追怪取消、施法/死亡屏障、数字键药品；保留拒绝/位置确认/显示队列诊断 | R17 已发布；三职业攻击后移动/保存烟测通过；连续受击和原生验收待办 |
| 2 | 完整经典地图和后期打宝 | 按 `platinum_176` 路线补齐石墓、祖玛、赤月及所需过渡地图；地图图像/二进制/小地图/入口闭包；正常到达、Boss、装备和技能书获取证据 | 208 runtime/209 原生实物资源已验证并集成，R18 已打包但尚未发布；具名条件入口根复验 13+2+44 通过；76 跳只为计划，正常联网/Boss/掉落与真人验收待办 |
| 3 | 原版经验与掉落归属 | 比对 Crystal EXPOwner/LastHitter；共享玩家/宠物参与、超时、组队/Boss、死亡及保存恢复；不改变用户明确的自定义日常奖励 | 普通怪同图首击/v5 及独立在线 Dead 已实现；死亡毒杀升级/复活/保存链限定检查通过；跨图/组队/Boss/地面期限与新版本联网保存待办 |
| 4 | 道士宝宝 PK | 真实召唤物攻击玩家、护主反击、主人攻击模式/组队/公会/安全区资格；身份代、伤害/死亡、离区/保存生命周期 | 打怪已实现；完整宠物 PvP 待实现 |
| 5 | 行会管理与普通行会战 | 成员/职位权限/公告/战争正常指令；杀怪行会经验；双账号、权限失败、并发、持久化/重启验证 | 建会/邀请/银行/Buff/沙巴克已有；公告及职位改名权威已实现；改名 21 项检查通过、根复验重叠 6 项通过，尚未发布；原生状态回复/名单/等待、踢人/普通战争/杀怪经验待办 |
| 6 | 挖矿与升级武器 | 导入原版矿区；普通挥镐/材料；Windows NPC 存入、精炼、检查、领取入口；完整物品身份、原版概率和失败/保存恢复 | 矿区和客户端精炼入口缺失；后台部分已有 |
| 7 | 原版地图活动和事件 | 逐项列出并落实当前源数据实际事件及条件/动作；正常触发、失败分支、奖励和恢复；不能用自定义日常代替 | 通用事件仅导入源数据 |
| 8 | 后期扩展 | 依次评审全服寄售、夫妻召回/婚戒、英雄、装备觉醒；正常 Windows 入口、共享权威、跨账号资产事务和保存恢复 | 原型/部分逻辑已有；经典阶段之后推进 |

扩展职业和其他 Crystal 攻城模式不是本轮经典三职业必需项。普通交易、仓库、
师徒、玩家 PK、Boss 贡献分配、行会基础和沙巴克已有实现，不按旧清单重做。

## 第一轮：发布拥挤逃跑修复

- [x] 保留已推送修复与源码哈希、失败记录、1,323 项有范围的通过证据。
- [x] 核对当前在线会话、服务版本、发布输入、签名和回滚路径。
- [x] 构建干净源码对应的 Windows 客户端与配套 Linux Gateway。
- [x] 生成并独立验证下一 Candidate、实际签名包文件更新/回滚与在线安装器；核对资源与引擎身份。
- [x] 在隔离服务完成正常认证协议烟测，再执行有备份、可回滚的配套发布。
- [x] 公网清单/签名/文件哈希与健康版本相符；实际变化和下载量如实登记。
- [ ] 记录正常三职业多人受击/逃跑、原版控制及正常登出/保存证据。
- [ ] 真人经典移动/NewMove、多怪留出口/全包围打开出口、攻击/施法/死亡画面验收。

第一轮既有验证见 [拥挤逃跑修复](CROWDED-COMBAT-ESCAPE-20261003.md)；不重复已通过
的套件，除非新增代码、构建差异、失败或其他未解决问题需要回归。

配套交付见 [R17 证据](generated/player-qa/crowded-combat-escape-20261003/release-r17/README.md)。
实际 R16→R17 更新仅下载 3 个游戏文件，素材和引擎未变化；111,064,197 字节载荷，
含元数据 135,432,721 字节。三职业烟测是攻击鹿之后跑动，玩家未受到怪物攻击；
不能据此关闭连续受击、施法/死亡屏障或原生鼠标/画面 Gate。新 209 图资源不在 R17 内。

## 发布与并行约束

协调者负责集成、全局文档和发布；每个高冲突文件每轮只有一个代码写入者。
先安排只读源行为、发布和资源探索；明确写集后再实施。

保留主工作目录未提交改动、玩家存档、原始服务和各版本失败证据。用户此前要求
保持 F 盘安装目录现状，该约束继续有效：不停止其他游戏、不替换本机安装文件、
不修复/删除更新日志。远端发布先核对在线会话；有人在线时先完成可审阅的包与
隔离验证，等待正常保存退出，不强杀角色。当前已知凭据只通过既有私密配置读取，
不复制进仓库、日志或聊天。

每轮完成代码后更新对应前后端证据、任务队列与路线图，再提交并推送。每个阶段
分别记录 source / tests / packaged / published / installed / humanAccepted，不能
以某一列替代整个阶段完成。

## 当前下一轮

地图先验证正常步行的三职业主线往返，再独立验证 Stone 条件房、Boss 和真实物品/技能书获取。
D10061 的 NeedMove 入口在供源脚本中没有正常绑定，仍记录明确缺口；D71653 不制造入口。
Stone/BigTaoist 两个具名旧脚本已完成全来源哈希限定的兼容修复与根代理复验，
没有全局改数字状态或把 Available 当成已接任务。76 跳计划不代替实际联网路线；
古墓内部房间也没有原版外出步行边。随机未掉落、入口失败、死亡和超时全部保留。
[修复与边界](CLASSIC-LATE-ROUTES-20261004.md)。

死亡单人毒杀在 Zone 产生奖励后，个人升级补满 HP 可能经 Gateway 同步错误地清掉 Dead；
登出时消费排队奖励还存在提前快照覆盖升级 vitals 的路径。下一轮以原版同在线身份的
独立 Dead 状态为依据，验证普通到账/升级、重复投递、保存失败及正常退出/复活。
此项已完成限定源码修复及 15 项根复验；新配套版本的实际联网 Gate 仍开放，不能由 Zone 归属或 in-process 数量推导整条上线链完成。

R17 的独立 v4 实测已用正常购书、学习、重登后的法师/道士复现死亡施法：
法师 HP0 后火球仍扣 3 MP、改变方向；同段目标损失 8 HP 的攻击者归因不明。道士 HP0 后治愈术
也扣 3 MP。两者没有施法回包，但都不能判为拒绝成功。原冻结工具误报通过的结果
保留；新离线分类和死亡状态修复分别验证。该短窗口没有达到七怪连续正伤害的
资格，未执行合格压力下的逃跑命令，不据此判断逃跑算法成功或失败。

法师目标8HP损失之前还有同目标 dead Attack，包内没有攻击来源，不能归因为
特定火球。v5离线重分类依据两个明确扣蓝窗口保留失败，原始误报不覆盖；源版
死亡升级补满HP也不代表复活。未来固定版验收工具需分别处理这些归因边界。
[原始实测与离线更正](R17-DEAD-CAST-AUDIT-20261004.md)。

[共享公告](SHARED-GUILD-NOTICE-20261004.md)按当前职位权限写入并耐久提交后，
只更新同一行会在线成员。准备行会、普通包与 File 重开是本轮机械证据；整套行会、
原生公告点击和 SQL 重启仍分别验收。

R17 隔离怪群测试使用新认证角色和明示 prepared 等级/位置，不改现有玩家。工具首次因
装备编号字段假设错误被守卫拦截，记录保留；修正后仍必须实际证明购书/施法、受击
压力、逃跑确认、停止后无漂移与死亡屏障，结构完成或 exit0 不能替代这些结果。
