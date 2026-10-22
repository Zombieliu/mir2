# 玩家挖矿→鉴定→切割→饰品协议验收（2026-10-18）

## 结论与执行范围

已完成两批明确绑定源码的本地FileSource普通协议验收：`55e50e27f7f4af82f0cea679d49aa1e65a245930` / runtime07的自然铜矿→铜戒指→装备重登；修复后 `95146ae328b243465c91b368dbe076d9eaa457b2` / runtime08的天然银矿首次鉴定、等待后首次切割→实际玉晶产物→新WebSocket重登。

**这是用户要求的脚本协议验收，不是GUI验收。** 初链真实Cut报价过期和脚本观察错误均保留；新修复首报价已复验。天然银矿实际切出JadeCrystal904×1，故 `jewelryStatus=notApplicable`，没有制作银镯，也没有补造材料。各阶段事实、具名覆盖集合及证据哈希见 [outcomes.json](generated/player-qa/mining-jewelry-20261018/outcomes.json)，不将旧源码结果转记到新提交。

历史受控前置为等级12、Gold10000；矿镐按正常商店价格2500购买后余额7500。10000不是本轮自然赚取的金币。本轮流程未发放矿石/饰品、未发送 MoveTo/Stage5/QA指令，未直接改写游戏状态。已记录的正常命令为版本校验、登录、StartGame、walk/run、attackDirection、interact、SelectNpcDialog和退出。

## 初链实际证据

| 阶段 | 实际结果 | 原始记录 |
|---|---|---|
| 自然挖矿 | D401(24,181)，106次普通 `attackDirection`；得到 Copper824，UID54788116，纯度9；Silver826，UID4，纯度13；Gold保持7500 | `mining01/mining-result.json`、`natural-ores-ready.json`、events |
| 普通回城与封存 | walk/run离开D401回地图0，交互比奇Bill19；铜矿封存为RawStone50001，公开菜单serial1，同UID54788116；费用0 | `workshops01`～`workshops03` 的公开快照与events |
| 鉴定 | 普通已显示NPC选项完成鉴定，扣100，Gold7500→7400；公开页面给出鉴定提示 | `workshops03/copper-appraised.json` |
| 首次切割 | 真实 stale quote 被拒，仍是RawStone50001，Gold仍7400，未扣1500 | `workshops03/copper-cut.json`、events |
| 刷新后切割 | 重新读取当前页面并选择Cut，扣1500，Gold7400→5900；产物为Copper824，同UID54788116 | `workshops04/stone-chain-result.json`、`copper-cut.json` |
| 制作饰品 | 普通移动到Alice33，选择铜戒指配方/制作；扣300，Gold5900→5600；Copper824重分类为原版CopperRing404，同UID54788116 | `jewelry01/copper-ring-made.json` |
| 装备与重登 | 装备落到 `ringLeft` 后，`relogin01` 普通login/StartGame确认戒指仍在、Gold5600；公开stat5（MaxDC）10→11 | `relogin01/result.json`、`ring-and-stats-persisted.json` |

矿石来源以挖矿前无这两件矿石的公开快照、106次攻击命令和挖矿后新增矿石共同确认。serial1来自NPC公开菜单链接，不要求世界快照暴露原石serial；未读取sealed private outcome。初链结束时银矿尚未消费；后续runtime08实际产物见下节，仍未把银镯制作记为通过。

`jewelry01` 在Equip动作后因BigInt日志序列化崩溃，没有完整成功结果，也没有落下该条Equip的events记录。装备已落库的结论由随后独立的 `relogin01` 公开装备/金币/属性状态确认；不伪造缺失的命令日志。该证明是普通重新登录，未扩展为进程冷重启或PostgreSQL验收。

## 初链金币核对

| 动作 | 本次变化 | 余额 |
|---|---:|---:|
| 历史受控初始金币 | 前置 | 10000 |
| 普通购买矿镐 | -2500 | 7500 |
| 自然挖矿、普通移动 | 0 | 7500 |
| 封存 | 0 | 7500 |
| 鉴定 | -100 | 7400 |
| 首次stale Cut被拒 | 0 | 7400 |
| 刷新后成功切割 | -1500 | 5900 |
| 制作CopperRing404 | -300 | 5600 |
| 装备与重新登录 | 0 | 5600 |

总支出4400，其中矿镐2500、鉴定/切割/制作合计1900；没有把矿石或历史测试金币记作自然金币收益。

## 修复后runtime08银矿实测

实际运行源码 `95146ae328b243465c91b368dbe076d9eaa457b2`。runtime07原服务正常stdin shutdown、exit0、日志复制完成后，将整个File库原字节复制至独立runtime08；源、目标均32975字节，SHA-256均为 `996373275de31301a8fce44cab2385860009eaf6dd5f851c7066d84eafce6496`。migration receipt记录未解析数据库、未改数据库字段、未发等级/金币/物品/位置前置，也未读取私密原石结果；旧状态和旧凭证未修改。

| 动作 | 实际首次执行结果 | Gold |
|---|---|---:|
| 初始天然银矿 | Silver826，UID4，纯度13，沿用初链自然产出 | 5600 |
| 封存 | RawStone50002，公开serial2，同UID4，费用0 | 5600 |
| 首次Appraise | 扣250，普通公开鉴定提示 | 5350 |
| 等待后首次Cut | 等待3202ms后首次点击即扣2500；同UID4真实产物为JadeCrystal904×1 | 2850 |
| 新WS普通重登 | UID4玉晶、原ringLeft UID54788116、Gold2850、stat5 MaxDC11完整保持；未发经济命令 | 2850 |

`silver02`仅发送封存/鉴定/切割三条经济命令，各一次，`noEconomicRetry=true`。产物按普通公开GainedItem及owner inventory确认，不预读隐藏结果；产物不是铜或银，首饰分支准确记为notApplicable，不能声称银镯制作成功。初链铜戒指仍成立。

本协议等待使用queueLimit1，3202ms期间 `worldsObserved=0`、`dirtyIdleCheckpointClaimed=false`，仅证明idle稳定性，不声称这一段触发了dirty checkpoint。强制第三次保存和dirty-idle保存由Gateway回归单独覆盖。

`silver-relogin02`是全新WebSocket上的普通login/StartGame，并非服务进程冷重启；没有重复经济命令，公开属性difference为空。最终公开proof及audit确认先前公开的serial1、serial2均为 `lifecycle=cut / revision=2`，对应carrier UID54788116、UID4，未读取隐藏outcome。

runtime08随后正常停服，stop receipt的 `exited`、`logsComplete`、`cleanFilePublicationAfter`、`oldStateAndCredentialsUnchanged` 均true，exit0。停服前公开快照和停服后文件收据按各自时点保留。银链新增支出2750，连同初链4400，总支出7150，余额2850。

## 原失败与恢复记录

| Run | 保留的失败 | 对结果的限制 |
|---|---|---|
| workshops01 | 初始NPCResponse等待条件错误，events记录 `Awaited packet timeout` | 该Run不计完整工坊成功；回城与Gold7500证据保留 |
| workshops02 | 封存后误要求world附带public serial，记录 `seal did not attach public serial` | 是观察字段假设错误，不能重发封存或凭空补serial；后续公开NPC菜单已能选择serial1 |
| workshops03 | 首次Cut后仍是原石，记录 `cut did not replace raw-stone template`；公开页面提示Bag/stone已变化 | 是真实报价时序缺陷；前后Gold均7400，刷新后的成功不抹去这次失败 |
| jewelry01 | Equip后的BigInt日志序列化崩溃 | 脚本本身不记完整成功；由relogin01另行确认装备持久结果 |

普通movement、公开NPC选项与重新登录贯穿这些分步会话；没有把多次会话拼成“单次脚本全绿”。

## 保存顺序修复与最终回归覆盖

首次Cut过期确认为Web第三次保存推进revision：报价已发出后再次保存使其变旧。修复保留最终保存、CAS、owner/generation、当前已显示菜单及Unknown围栏，让最终保存后的公开报价可首次执行；没有放宽revision或重试经济操作。

| 提交 | 范围 |
|---|---|
| `4ef821c50ac0802b87f7a171efefd6f126a2dbd2` | 产品保存顺序修复 |
| `9e4c822ce42cad6667f83af245b9d08dd76876bf` | 仅测试：使用public API |
| `2f6dfc05d405690e96ce31bea115eda19f97e610` | 仅测试：合法且多样的recovery key |
| `95146ae328b243465c91b368dbe076d9eaa457b2` | 仅测试：离线Source fixture、本地60s ack与idle baseline |

据Root冻结复核，产品源码自4ef未变；Simulation源码及测试在9e4 focused通过后亦未改。旧SHA focused按原调用列示，不冒充在最终HEAD重新执行。

| 实际调用 | 实际HEAD | pass/fail/ignore | filtered | 说明 |
|---|---|---:|---:|---|
| simulation-focused02 | 9e4c822ce4 | 4/0/0 | 2955 | 97.93s，后续Simulation源/测试未改 |
| gateway-focused04 | 95146ae328 | 4/0/0 | 1180 | 117.71s，强制第三save、idle及失败/Unknown围栏 |
| simulation-regression04 | 95146ae328 | 74/0/0 | 2885 | 1019.26s，原Guard接受，pipeline计数未通过 |
| simulation-coverage-gap05 | 95146ae328 | 11/0/0 | 2948 | 3.00s，仅补遗漏11项 |
| gateway-regression05 | 95146ae328 | 10/0/0 | 1174 | 66.13s，low_latency7 + shared appearance3 |

成功调用均原Guard/audit accepted、1456编译输入、0 drift。pipeline04仍为 `failed-count`：较窄stone_过滤实际74项，覆盖旧83中的72项并新增2项，漏11个旧广匹配。pipeline05未重跑74、未降低Guard或覆盖门槛，仅补11；具名PASS集合 `74∩11=空`、`74∪11=85`，旧83全部覆盖，新增恰为Stone及Atelier完整保存后首报价的2项。85名清单见outcomes，各组不累计为全Simulation或全项目绿灯。

`gateway-build04`在95146真实exit0/accepted，产物123378688字节，SHA-256 `e7255fdbc946469fda038e57674fa38922121e82e19eff0ed2eb3b74c05d5b57`。本轮不宣称重新完成Native GUI或Windows完整发布验收。

以下编号属于 `.mir2-player-qa/workshop-checkpoint-20261018/`，原失败保留：pipeline01为Gateway E0624编译失败；pipeline02为测试recovery key校验失败及cascade；pipeline03为测试启动10s timeout和live File fixture race；pipeline04仍failed-count，74项原Guard通过不等于pipeline整体成功。pipeline05为complete，精确补11并通过Gateway10。此前fashion-shopfix-20261017的pipeline03零测试失败另按其原目录及HEAD保留。

## 证据与尚未验收范围

初链原证据位于仓库相对路径 `.mir2-player-qa/fashion-shopfix-20261017/scripted-acceptance-20261018/`；修复复验、公开proof、迁移/停止收据及回归位于 `.mir2-player-qa/workshop-checkpoint-20261018/`。完整原文件仍ignored。[脱敏摘要](generated/player-qa/mining-jewelry-20261018/outcomes.json) 固化事实、具名覆盖集合及字节数/SHA-256，不复制账号凭证、恢复资料、完整请求target或私密原石结果。

当前接受初链铜矿→铜戒指→装备重登，以及修复后天然银矿→首次鉴定/切割→实际玉晶产物→重登的本地普通协议结果；没有银镯制作结果。GUI、人类最终验收、时装、真PostgreSQL及正式发布仍未通过；线上部署暂停，未部署，`Candidate100=false`，总体百分比不变。
