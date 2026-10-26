# 普通经济准备与 500 CCU 目标

2026-10-27（北京时间），用户授权补齐持续负载准备，并将最高目标扩到 **500 CCU**。当前仍为 P1 通过、P0 未通过、P2 未启动；此记录不声明新容量通过。此前[V13 原失败](../capacity-online-presence-publication-v1/README.md)及历史正式 2/10 通过、25 战斗频率失败保持。

本轮先独立验证普通角色通过 Deer 狩猎、harvest、拾取 Venison、NPC 售卖和 HP 药购买的实际闭环，采用有限准备时间、命令与重试预算，不授予物品、改存档、开启 QA 或自动复活。普通新角色初始金币为 0；默认原版 Q1 仅额外一瓶 HP 药、无金币，不能假设它足够支持持续压测。已有普通经济 helper 可复用，但源码/模拟测试不能替代本轮真实协议准备验收。

旧默认 held-only profile 保留。新 ordinary-economic-v1 显式启用并独立记录准备阶段、真实收据及新 cohort 的激活时刻；准备采用自己的有限超时，不修改原测量动作 2 秒证明窗、3 commands/s、滚动活动率、AOI 或死亡保护。已激活 cohort 的活动计数不得因新角色准备而重置或隐藏。药品消耗证明与治疗生效证明分别记录，备足一批药不等于两小时可持续。

最高目标阶梯为 2 / 10 / 25 / 50 / 100 / 250 / 500。先完成原 P0 的 100 人基线，再随 Cell、多核、多机推进 250/500；保持每档 300 秒和目标档至少 7200 秒长稳、恢复与保存验收。500 目标须显式选择，不静默扩大旧默认或放开资源保护。

暂按 500 在线、50 名持续战斗者设计，最终战斗比例正向用户确认；在线、同图、同屏及持续战斗人数分开报告。分散与热点负载各自证明；地图足迹和普通目标供给不足时失败，不重复坐标、刷怪或减少声明的战斗者来制造通过。

更正范围：主源旧逐人激活路径与 V13 外部诊断执行器不同。V13 已在全员入场前 paused，并在激活后执行 settle；不能将其全部 343.454 秒准备时间归为早入场角色持续巡逻。V13 最终 HP0/18、金币0、HP 药0 是实证，致命攻击来源和 HP 调度饥饿尚未证明。

本轮实现已具备：有界普通经济模块、真实接收序号/单调时间的标量证明、独立准备阶段、每 actor 不同 staging 点、全员新鲜检查后同步激活，以及 HP 恢复先于普通动作的公平调度。HP 证据等待仍计入原 offered 分母，不增加 completion。旧阶段内仍仅使用已持药品，尚无测量内自动购物；一批 12 瓶仅是短测准备目标，不宣称两小时供给已解决。

`d926d5bcd` 的 Root 新鲜 source-bound [完整检查](pilot-v1/root-focused-receipt-v1.json)通过：容量工具 284/284、普通经济/导航相邻 301/301，27 个受检输入前后相同；0 failed/cancelled/skipped/todo。各组口径明确，不把 worker 的重复聚焦运行叠加为额外覆盖。准备 before/after 的生命证据来自新鲜 worldSnapshot；live 低血量控制允许保守百分比估计，不能把估计值当作新鲜精确 HP。

## 第一次真实准备试运行：未通过

[原报告](pilot-v1/1793039455345-4130b3d1.report.json)在准备阶段约 3.566 秒中止，错误为 `Cannot read properties of null (reading 'npcObjectId')`；[原执行收据](pilot-v1/pilot-receipt-v1.json)保留 `ok=false`、0 prepared/0 activation、无测量。不能将 585 项工具测试通过解释为真实准备闭环通过。

源审查确认普通 `@BuySell/@Sell` 会建立服务并可正常关闭文本对话窗，原模块选择服务后却仍盲读 `activeNpcDialog`。修复保留此前新鲜同 NPC 对话及链接、这次新鲜服务包，并重新验证 live NPC/owner/map；允许合法空文本窗，拒绝不同 NPC、旧包、缺包及身份变化。真实 samples 未保留 NPC 收件窗口，不能声称已有完整逐包时间序列；该诊断由异常位置及服务状态源码支持。新增失败状态记录会继续抛原错误，不激活或增加成功计数。

[正常清理](pilot-v1/cleanup-v1.json)与[Root 独立 OS 检查](pilot-v1/root-independent-cleanup-v1.json)确认受控 PID71204 消失、17500/17510 端口释放。实际运行复用 `7b43caa41` 的 immutable Release V5，55 个 Rust 输入严格相同；没有新 Rust 构建或新鲜 Rust 测试。新 JavaScript 的 30 个本地静态依赖及数据单独冻结，见[冻结来源](pilot-v1/frozen-source-v1.json)与[复制清单](pilot-v1/copied-evidence-manifest.json)。

## 第二次真实准备试运行：未通过

`55cd890d6` 的新鲜[容量工具295/295、普通相邻301/301](pilot-v2/root-focused-receipt-v1.json)通过，27 个输入前后相同、无失败或跳过。[第二次原报告](pilot-v2/1793040354572-617efd90.report.json)仍在准备阶段停止：`timed out waiting for Deer corpse ... funding harvest`，0 prepared、1 failed、0 activation，无测量，见[原收据](pilot-v2/pilot-receipt-v1.json)。

[匿名原样本](pilot-v2/1793040354572-617efd90.samples.jsonl)中的失败状态显示初始 HP18/18、Gold0、HP药1、Venison0；模块耗时22.025秒，记录17次模块发送调用，其中10次attackV2、1次harvest，售卖和购买均0，after为空。模块发送计数不是服务端收件证据，现有记录没有逐包采集结果，不据此断言本次harvest是否获准或最终生命/库存值。

只读源审查确认现有共享尸体桥、同object ID与位置/朝向同步已实现；鹿原规则是五次剥皮、第六次库存转移，前几次没有肉可以正常发生。旧funding helper只捕获`Timeout waiting for ...`，容量客户端的`NAME timed out waiting for ...`不匹配，导致仅一次采集后退出。完整snapshot仍在client.snapshot，不能把本次失败归因于event history裁剪。

最小修复仅在工具层增加真正到期的类型化等待标记，并在准备facade按精确标记/label适配旧helper；身份、健康、安全停止、连接及全局600秒预算错误不变为可重试超时。新增真实六次采集形态测试，保留18次采集/三鹿、同格拾取和新鲜经济证明；不修改Rust、掉落或剥皮规则。

[正常清理](pilot-v2/cleanup-v1.json)及[独立OS复核](pilot-v2/root-independent-cleanup-v1.json)确认PID63968消失、两端口释放。来源、原判定与复制摘要见[第二次清单](pilot-v2/copied-evidence-manifest.json)。后续结果如下；P0 未通过，500 CCU 和两小时可持续性未证明。

## 第三次真实准备试运行：两人买足药，一人 ready

`f1d4a00d5` 的新鲜[变更路径门禁](pilot-v3/root-focused-receipt-v1.json)为补给模块48/48、主驱动受影响等待与准备32/32、普通经济/导航相邻301/301，共381个本轮实际受检项，27输入前后相同、无失败或跳过；不把第二轮的完整295项追认为本次完整容量回归。

[原样本](pilot-v3/1793041026401-c2a7a2b4.samples.jsonl)证明两名角色各正常狩猎3鹿、发送18次harvest，各买11瓶HP药，最终各持12瓶；本次实际走第六次harvest直接入包路径，未发送pickUp，不能把同格拾取fixture当作本次live覆盖。两人均从HP18/18、Gold0、药1开始，最终HP24/24，余额65/232；角色A准备141.117秒并ready，角色B准备147.525秒在`(288,602)`因`hostile near staging`失败。[原报告](pilot-v3/1793041026401-c2a7a2b4.report.json)与[执行收据](pilot-v3/pilot-receipt-v1.json)保持失败：严格ready仅1、激活0、无测量。

[匿名现场记录](pilot-v3/public-staging-threat-observation-v1.json)指出失败前28ms的原录制帧1075包含存活、HP20、hostile的Scarecrow206604，位置`(280,602)`，距两待命点5/8格。原待命点在村庄安全区外，遂把下一轮待命点独立设为`(286,616)/(289,616)`，测量homes仍为`(285,602)/(288,602)`，通过原普通导航进入；没有传送、降低敌怪距离或改战斗门槛。[正常清理](pilot-v3/cleanup-v1.json)及[独立OS复核](pilot-v3/root-independent-cleanup-v1.json)确认PID57964消失、两端口释放；原证据与来源见[复制清单](pilot-v3/copied-evidence-manifest.json)。

## 第四次真实准备试运行：权威安全区状态与旧待命判定冲突

第四轮仍为2人、30秒阶段加30秒soak的功能探针，未达到正式300秒或两小时；源码仍为`f1d4a00d5`，30个本地静态输入与55个Rust输入重新逐字节核对，复用同HEAD的381项资格及既有V5二进制，没有新增Rust构建或新鲜Rust测试。

[原报告](pilot-v4/1793042214886-d262ad8a.report.json)在第一名角色已正常买足12瓶后，于安全区内`(286,616)`再次报`hostile near staging`，0 ready/0 activation、无测量。[更正后的匿名现场提取](pilot-v4/public-staging-threat-observation-v2.json)保留失败前75ms的帧524：`world.inSafeZone=true`，同时存在3只活的、HP20、hostile的Scarecrow，距离为1/7/7。第一次提取误用时间字段而得到空窗口，其[无效v1输出](pilot-v4/public-staging-threat-observation-v1.json)保留；只使用改按`capturedAtMs`取得的v2作为现场证据。

该结果不能再归因于待命点在安全区外，也不能通过换点或名字豁免来宣称解决。公开WorldSnapshot已有服务端Zone覆写的权威`inSafeZone`；旧补给标量没有采集它，仅按hostile及距离拒绝。下一候选须把同一新鲜worldSnapshot的owner/name/map/x/y、实际收件序号与单调时间绑定到安全区状态，在prepare和全cohort屏障同时验证；不能使用移动后的残留布尔值，也不能把普通怪物安全区保护描述为毒/已释放效果等全部伤害免疫。生命、死亡、身份与原测量门槛保持。

[正常清理](pilot-v4/cleanup-v1.json)及[独立OS复核](pilot-v4/root-independent-cleanup-v1.json)确认PID69992消失、两端口释放。原输入、失败和提取更正见[第四轮清单](pilot-v4/copied-evidence-manifest.json)。后续以正常保存、原字节保留的普通角色池避免每次重建补给；复用角色必须通过普通重登和新鲜readiness，不能继承旧ready或冒充新0角色入场。本轮仍未完成25/50/100人数档、100人两小时及500目标。

修复已实现于补给工具：prepare与assertReady共用最新实际worldSnapshot收据，不回退到较旧true；新快照即使因无效标量而被拒收，也会使较旧安全区证明失效。只有同owner/name/map/待命坐标、实际收件序号和单调时间有效的新鲜`inSafeZone===true`可声明普通怪物安全区保护；false/缺失/非布尔状态仍执行原8格hostile检查。现有公共Poisoned/ObjectPoisoned折入的u16中毒状态及owner Dazed已知为异常时拒绝。没有新增Rust/生产协议字段、命令、额外刷新、阈值或药品；新鲜Root完整资格与现场结果如下。

## 普通角色库重登：库存与待命通过，短测战斗供给未通过

`e73d04d25` 的新鲜[Root完整门禁](bank-relogin-v1/root-focused-receipt-v1.json)通过容量工具330/330、普通经济/导航相邻301/301，27输入前后相同、无失败或跳过；重复worker聚焦项不额外累加。30个本地静态输入及55个Rust输入再次冻结，复用既有V5，没有新Rust构建或新鲜Rust测试，见[执行冻结](bank-relogin-v1/frozen-source-v1.json)。

这次独立私有目录原字节复制第三轮正常保存的完整账号Store、对应pool、原登录预算和4个密钥。旧recovery只有4个控制文件、无journal payload；原树完整保留，新活动recovery由正常Gateway建立绑定新路径的sentinel，没有修改旧sentinel或放宽校验。没有seeder、金币/物品注入、合并角色、手改pool或重置auth账本。[复制收据](bank-relogin-v1/bank-copy-receipt-v1.json)只有文件pin，私有文件内容不进仓库。

[真实重登收据](bank-relogin-v1/pilot-receipt-v1.json)证明两人各以原name/characterIndex普通重登，before已持12药、HP24/24、Gold65/232；本轮purchased=0、soldVenison=0、funding明确为null。两人在不同安全区待命点收到同位置、同owner/name/map的新鲜`inSafeZone=true`，附近各3只hostile仍正常ready，随后一次屏障激活2人。它覆盖第四轮误拦修复及库存持久化复用；源第三轮1ready/1failed不继承，本轮也不虚称重新完成0金币筹款。

[原报告](bank-relogin-v1/1793043370759-685834a8.report.json)保持`ok=false`：stage2实测30.005秒通过，战斗角色18次发起/18次正伤害、总伤害50；soak实测30.093秒时同角色40次计划均完成，但39次为移动、0攻击/0命中，判为`scenario-insufficient`。移动、聊天、AOI、fanout及连续性检查通过。见[匿名指标](bank-relogin-v1/public-failure-metrics-v1.json)；仅能据此说该窗口没有合格战斗攻击，尚不能归因为Deer耗尽、观察者不可用或服务器资源上限。30秒通过不能替代正式300秒档。

本轮没有HP恢复日志，不能据此认定测量末药量不变或可持续7200秒。现压缩日志与终点存档不足以还原分钟HP/药耗；后续沿现有实际收件补充有界标量，不增加命令、不暂停或隐藏offered，并继续用原正式场景/目标见证约束验证。扩容准备需要独立普通角色库流程，现有probe最多10人，不能把单阶段25/50/100伪装成probe。

[正常清理](bank-relogin-v1/cleanup-v1.json)及[独立OS复核](bank-relogin-v1/root-independent-cleanup-v1.json)确认PID68696消失、两端口释放。原判定、原样本、来源与复制摘要见[重登清单](bank-relogin-v1/copied-evidence-manifest.json)。当前仍为P1通过、P0未完成、P2未启动；尚无新25/50/100/500或两小时容量通过。

## 独立准备流水线与被动供给观测候选

新增`apps/web/scripts/playtest-capacity-prepare.mjs`独立普通准备入口，单worker复用原CapacityClient、持久登录预算、ordinaryLogin、prepareCapacityActor及正常drain；不启动engine、激活cohort或测量CCU。完整100账号pool与未准备尾部保留，角色逐名真实补给后，末次正常退出快照的身份、地图和位置必须与新鲜readiness一致，才计入preparedIndex。准备固定V2攻击，不能默默退回legacy。25/50/100只记准备进度；整次正常关闭Gateway后才有冷保存资格，所有角色持久化重登仍须后续正式流程验证。

准备场景声明安全区内335个静态可走、非NPC/传送/出生格候选，100个为优先点。补给完成后只选择一次当前新鲜快照可见、未被活实体占用、未被本轮已保存角色使用的点，再冻结到该helper的准备和readiness检查；实际点写入preparedIndex，不能改存档位置、强制占位或把335候选称为335 CCU。旧固定点调用行为保留；600秒、8次狩猎和命令预算不因选点重置。

容量主流程新增被动sustain标量：只沿已收到的worldSnapshot记录精确HP/maxHP、完整bag+belt的HP658总量、金币、实际安全区布尔值及owner/name/map/连接/收件时间；缺失保持未知，不保留完整快照。仅激活、实际分钟、正常drain边界输出，净库存变化、成功用药ACK、已证明消费量与观察到的HP增加分开计数；不新增请求、暂停、完成量或SLO豁免。已有ordinaryLogin/drain仅导出供准备工具复用，原函数行为保持。

`5024c81236ed6cfccfd44126632d92ce6e0133c2` 的统一Root完整门禁已实际执行：容量与准备工具365项、普通经济与导航相邻301项全部通过，0失败/取消/跳过；29个输入前后字节一致。见[资格收据](bank-prepare-v1/root-focused-receipt-v1.json)和同目录原始测试日志。这666项是工具验证，不能当作666人或任何CCU验收。

## 首次百人角色库准备：完成3人，第4人目标资格失败

本轮使用完整100账号pool、单worker、V2攻击和每人12瓶HP658目标，未启动容量engine或激活cohort。前三名角色从初始HP18/18、药1、金币0开始，通过各3次Deer狩猎及普通出售、购买获得药12，正常保存到实际安全区坐标；各自准备耗时139.090、154.634、140.344秒。这是3个角色的准备结果，整轮仍未通过。

第4名角色在90.561秒、77条命令后因`ordinary supplies: only live Deer may be hunted`停止；[原始报告](bank-prepare-v1/1793046276153-143c0f31.preparation-report.json)保持`ok=false`、`preparedCount=3`，完整100账号pool保留。仅此错误还不能判定为服务器容量、鹿被其他人击杀或过期快照；目标选择路径按原样本核查。没有25/50/100冷保存完成、普通100人持久化重登验证或任何人数档测量通过。

[正常清理](bank-prepare-v1/cleanup-v1.json)和[Root独立OS检查](bank-prepare-v1/root-independent-cleanup-v1.json)确认PID72788消失、17500/17510端口释放、监控停止、六计数归零及冻结来源未变。27份公开证据原字节复制共2,906,567字节，见[清单](bank-prepare-v1/copied-evidence-manifest.json)；私有Store、完整pool、原登录账本、秘密内容和完整录像留在隔离环境，不提交。后续复用正常冷关闭的部分角色库，继续普通补给与保存，不清空原登录预算或重复赠送物品。

随后补给适配器把本模块产生的精确目标失效映射到原funding有限重选路径；非Deer阻路怪仍禁止攻击，不计死亡或收获，不重置8次狩猎尝试、600秒及命令预算。攻击入队和实际限速等待后的发送点均检查相同目标ID、monster/Deer、明确`dead=false`和有限正HP；缺失、畸形或歧义身份拒发。owner/name/map/health/transport变化仍终止。最多32条拒发标量记录目标ID/资格、实际收件序号及owner，不保留完整快照。模块90项聚焦回归通过；统一Root新门禁和部分角色库续跑仍待执行，不能将候选修复追认为首次百人准备成功。

## 第二次百人角色库准备：旧库存复用通过，第5人生命保护停止

`2444c9ba01978dcc3674a04859f5594ffaebac62` 的新Root门禁已实际通过387项容量/准备工具和301项相邻检查，0失败/取消/跳过，29个输入前后不变；见[新资格收据](bank-prepare-v2/root-focused-receipt-v1.json)及原始日志。本轮完整复制正常关闭的原Store、100账号pool、原登录账本和四个秘密；旧recovery控制文件仅原样归档，新活动目录由Gateway正常初始化，没有seed、清账本或覆盖loginNotBefore。完整Store101账号与受控pool100分别记录，不把额外账号计入负载。

前三名角色本轮普通重登后的库存仍满足12，且无新狩猎/出售/购买，`sourceFirst3InventoryReuse=true`；第4名完成普通补给并正常保存，preparedCount达到4。新目标拒发计数在这五名角色上均为0，不能把本轮进展宣称为真实触发了失效重选分支。第5名169.274秒、125条命令后触发原`owner/map/health changed`保护；[原报告](bank-prepare-v2/1793047839765-82a5e52b.preparation-report.json)保持`ok=false`，没有人数档测量。

[公开生命与近域证据](bank-prepare-v2/public-health-guard-evidence-v1.json)及[原始录制标量提取器](bank-prepare-v2/extract-public-health-guard-evidence-v1.mjs)确认第5名owner1000/姓名/map0/level1/maxHP18未变；位置343,593附近精确HP18→16→15→12，最后下降约154ms后失败，12/18低于70%。近域两只HookingCat距1/2，正在接近/攻击的Deer在343,592；health事件没有attacker ID，不能认定是哪只造成伤害。旁观录制的空inventory不用于推导其真实持药量。原funding搜索启用避怪半径2，但Deer接近/回退传0、其他普通导航默认0，仅避占格，需把已有导航避敌能力接入完整准备路径。下一轮若复用该低血存档，必须通过普通恢复及fresh生命检查，不能跳过保护或改Store。

[正常清理](bank-prepare-v2/cleanup-v1.json)及[独立OS检查](bank-prepare-v2/root-independent-cleanup-v1.json)确认PID61120消失、两端口释放及冻结来源未变。原判定与26份公开输入/结果/门禁日志（1,756,761字节）见[复制清单](bank-prepare-v2/copied-evidence-manifest.json)。继续定位普通狩猎路径的威胁暴露，原70%生命保护、600秒/8次/命令预算保持；P0未完成、P2未启动，尚无新25/50/100/500或两小时验收通过。
