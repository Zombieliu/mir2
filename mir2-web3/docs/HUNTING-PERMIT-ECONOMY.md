# 狩猎许可经济实现与验收

## 第十五阶段：固定估值材料转换（2026-10-21）

用户已选策划固定估值、手续费扣现有非绑定金币。主服现已实现普通 Bag 的 Bound 根整栈/部分转换：总估值 E 只扣 E 许可原子及 ceil(2%E) 非绑手续费，不生成金币；完整物品 checkpoint、钱包、许可、fee 销毁与不可变结果同 PG 提交。原单 Query、未决普通退出及冷登录恢复已接，旧 gold 不改，正式价格白名单为空且默认关闭。详见[材料服务端合同/伪代码](HUNTING-MATERIAL-CONVERSION.md)。

实际验证：policy/service24项普通、经济PG相关42项最终去重、Sim48项普通及6项实际PG（11个隔离schema）、wire43、Gateway35、Python21通过；真实WS3项/5个schema及相邻修理退出3项（含2项PG）通过。WS整批/部分/新配置冷登录来自189，真实未读回执后raw断线/resume/原Query及冷登录来自207；前两项普通路径未受最后native夹具调整影响。BI离线47案例/87断言/230配置检查及实际PG42案例/71规则值通过，44列视图DDL/测试数据全部ROLLBACK。早期编译、UID分配及各夹具修复失败记录均保留；不是同一个二进制或全工作区测试，未重新导入Grafana或验证通知。见[阶段十五证据](generated/hunting-economy/acceptance-20261021-material-conversion.json)。

Windows材料客户端、Runtime及背包界面正在独立第十六阶段接线，未纳入本轮后端提交范围，尚无材料EXE/GUI验收；第十二阶段修理实机结论原样保留。完整goal active、K=0.1及0.72/1.08许可不变，不提高Crystal/Candidate。剩余：Windows材料完整背包Applied/物理发送/GUI、其它生产来源和三层销毁、全服市场画像收益、通知、正式价格管理与迁移发布。无Docker/WSL/虚拟机或远程资产变更。


## 第十四阶段：首个真实绑定物品来源与冷加载保护（2026-10-21）

现有 newcomer-v2 NPC3 Jane 的 2110001 任务已接真实绑定物品来源：保留原职业武器/药水、100金币及100EXP设计，完整物品 checkpoint 与绑定金币共用一次 PG 提交。新增严格 v1 服务端 realm/账号/角色 index/PG incarnation/source 标记；旧 None 仍省略序列化、不重新分类。发放先完整规划，绑定与旧栈或不同来源不混堆；普通用药、穿卸装备、个人仓库、拆分及同角色 Hero 保留标记并核所有者。

交易、邮件、寄售/成交、NPC出售、租赁、行会仓库、主动/死亡掉落均在入口及相关最终确认前拒绝绑定根或隐藏子项。尚未支持的 NPC材料交换、精炼、钓鱼变形在首次消费前拒绝，不能称作已实现配方继承绑定。相关 NPC TAKE 全动作图先检查，失败即停止后续 GIVE；不是通用 NPC 多步事务重写。封印 Hero 不能隐藏绑定物品。

领取结果不明只查原单；发布完整库存须匹配私有原背包基线和角色代际，基线变化则保留提交凭证并要求重登，绝不覆盖当前库存。成功完整 StartGame 才清旧 actor 的 Pending，resume 保留。另已封闭真实冷加载漏洞：在 DB witness 绑定后、任何库存/属性加载前核六组个人/Hero 绑定树，换 realm 或移除配置拒绝进入；原 realm 保持、仅经济 policy.enabled=false 仍正常重登。

Root 完成 Sim115、Gateway118 原生构建及272项去重验收：新增普通32、实际PG新增8/既有7、Sim相邻186、旧存档11、Hero重登2、Gateway26。最终冷加载增量重跑三职业实际领取/冷登录、绑定32、旧存档/Hero13及网关26，详见[阶段十四证据](generated/hunting-economy/acceptance-20261021-bound-item-source.json)。98条Sim警告及Gateway依赖93/本体5条保留；早期夹具错误及一次120秒超时均保留，旧四场景日常用例未改断言，在130.52秒复核通过。并非所有用例都来自同一个二进制，证据按主轮与冷加载增量标明。

本轮PG使用隔离schema、合成初始账号/位置/配置及普通NPC/Finish/Equip/Login/StartGame代码，不是物理WS或Windows GUI验收；第十二阶段原生修理实机结论原样保留，不新增EXE/签名候选包或Crystal/Candidate百分比。旧无sidecar鱼竿装备转换的槽宽边界另待验证；本轮钓鱼夹具使用完整合法五槽载体。

用户本轮已明确材料转换计费：策划配置固定金币估值，许可按总估值折算，2%手续费扣玩家现有非绑定金币。下一队列实现版本化白名单估值、明确 Converted 来源、材料 Quote/Confirm/Query 同PG扣许可/手续费与完整物品存档、日结/导出聚合及真实Windows入口；未配置价格拒绝，不猜正式价格。材料估值不计销毁，不产生金币，只有手续费计日常销毁。该材料链本轮尚未实现。随后可沿现有2110002→2110003绑定书→2110004真实共享练习扩来源。共享挖矿当前没有普通可达产出入口，采尸亦未具备完整共享持久来源，不能以个人 helper 宣称上线。种植、其它来源、三层完整销毁、市场执行历史/画像/收益、通知及迁移发布继续开放。

完整 goal active、默认关闭、K=0.1、基准每日0.72/上限1.08许可不变。不运行Docker/WSL/虚拟机，不修改远程资产或重分类旧gold；六个保护输入逐字保持。以下阶段十三及更早正文保留历史范围；其中旧“材料估值待用户定义”已由本轮决策替代。


## 第十三阶段：共享地面完整实例 custody（2026-10-21）

本轮完成完整实例地面 custody 前置保障，没有新增材料绑定、矿产、估值或狩猎许可转换入口。

GroundDrop 的可选 v1 `serverState` 保存完整服务端 ItemState。普通玩家丢弃（含安全的部分数量）及死亡库存/装备掉落均在删除源物品前实际捕获；拾取对 Some 载体执行有界结构、精确已知字段形状、canonical key/count/UID/完整 UserItem 对照，非法、未知字段、超限或不一致直接拒绝，不回退模板。部分掉落清理完整实例/元数据中的待分配 UID，需新建接收栈时由既有 allocator 分配新 UID；有嵌套身份而不能安全拆分时拒绝。合栈须完整服务端功能身份一致，同 wire 但不同服务端字段不能合并。

None 序列化省略，保留旧 checkpoint v1/v2/v3/v4 的 root 与原 ground-drop digest 版本/域；内部 JSON、MessagePack、ZoneBase 保留载体，客户端视图及 spectator 删除整个 `exactItem`。没有新增材料绑定、矿产/种植产出、估值或许可转换入口，也没有新增客户端包或材料经济字段。

Root 串行验证 Sim build80、Gateway build82 成功（Sim 保留 96 条既有警告）；Sim 新增9项（81）、原 ground identity 15项、checkpoint 16项（含新增兼容1项）及 stack 24项（83）共64项，Gateway RPC 2项及 spectator 1项（84）共3项，合计67项唯一测试通过：新增10项、既有/增强57项。本轮没有新增 PG 或 GUI 执行。见[阶段十三证据](generated/hunting-economy/acceptance-20261021-ground-item-custody.json)。

测试边界：真实普通 DropItem 后，显式分别调用 Zone Claim、个人 Pickup、Zone Commit/Cancel，并验证满包、UID冲突、载体不匹配时双边不变；这不是 Gateway/PG 一体原子 claim 证明。完整递归实例经过普通 LogOut，再由全新文件配置/Session 普通 Login/StartGame 冷加载；仅证明文件存储边界。死亡测试直接调用既有 penalty helper，不是完整 PVP 致死链。Quest 仅验证 planner 的外层显示别名不覆盖 Some 内原实例字段，原 shared quest 掉落仍为 None。checkpoint/RPC 的 Some 使用 opaque marker 验证保留、摘要和擦除，不是合法 Some 的业务恢复验收。

第十二阶段 Windows 实机验收提交 `a4bc32f461` 及其[原生恢复证据](generated/hunting-economy/acceptance-20261021-native-repair-resume.json)原样保留，本轮不追加 Windows/客户端实机结论。完整 goal active、默认关闭、K=0.1、基准每日0.72/硬上限1.08许可保持；不提高 Candidate/Crystal 百分比。不运行 Docker/WSL/虚拟机，不修改远程资产或重分类旧 gold。

剩余队列：材料绑定传播及交易/邮件/NPC/合成等出站约束、材料估值与解绑定规则（待用户定义）；共享挖矿/种植及其他真实来源；日常/轻PVP/沙巴克三层完整销毁；全服市场、画像与收益；通知送达及迁移发布。下方第十二阶段及更早正文保留历史范围，不能按旧段落推断本轮完成度。

## 第十二阶段：Windows 原生修理恢复与重登（2026-10-21）

修理来源已获EXE75/Gateway73原生实机恢复证据：77轮真实NPC/Bag报价6、单Confirm；测试proxy省略一份已提交owner回执，Pending主动Close后物理断线/resume各1次，手动原Query 1次完成。普通logout后新EXE进程78轮普通Login/StartGame显示新钱包999994/0、旧gold 1234。独立PG终核结果/销毁各1条、金额6、耐久970/970、save_version17/revision16。见[阶段十二证据](generated/hunting-economy/acceptance-20261021-native-repair-resume.json)。

修复涵盖mapIndex、原恢复身份/角色及完整producer成功入gameplay队列后重新发布owner。Native43/43（含4项新增）、SimMap5/5、Gateway普通5/5、物理WS1/1和真实PG1/1分别通过，不累加旧轮筛选。旧66轮恢复失败保留；两个客户端与Gateway/proxy退出0，19541/19542无listener。此处是客户端新进程重登，Gateway未冷启，未强制服务端Unknown，UI未显示耐久数字；不是完整源码/资源闭包或签名候选包验收。

K=0.1、基准每日0.72/硬上限1.08许可、默认关闭及完整goal active保持，不提高Crystal/Candidate百分比。材料传播/估值（待用户定义）、种植挖矿等真实来源、维护/轻PVP/沙巴克销毁、全服市场画像收益、通知与迁移发布继续开放。无Docker、远程资产修改或旧金币重分类；下方旧结论保留历史范围。

## 第十一阶段：真实日常委托来源（2026-10-21）

现有NPC24新人日常委托已支持三选二+bonus绑定金币；三种组合各11,000基础绑定金币/20,000基础EXP，旧gold不变。独立DB午夜日槽、canonical整组资格、完整奖励同事务、最多一次跨期重规划与原日恢复已通过14项新增PG、31项既有PG、21项普通回归，见[阶段十一证据](generated/hunting-economy/acceptance-20261021-daily-quest-source.json)。

K=0.1、0.72/1.08许可、默认关闭和经济04:00结算保持。新Windows实机已经收费/销毁6、绑定999994及耐久970/970，但物理resume超时后正独立修复，仍无完整原生恢复结论。材料、剩余来源、全服销毁/指标与通知迁移继续开放；goal active、无Docker或远程修改。

## 第十阶段：任务来源登录恢复（2026-10-21）

一次性任务入账结果不明且原 Session 丢失后，普通登录刷新现能核对原私有领取槽与经济回执，仅加载当前完整存档和最新 Guild 状态后解自身冻结。密码、封禁、UUID 与外来 fence 隔离保持；不一致存档继续冻结。新增 PG 9/9、既有 PG 22/22、普通回归 8/8 通过，见[本轮证据](generated/hunting-economy/acceptance-20261021-quest-login-recovery.json)。没有 schema 或客户端变更。

下一项为现有 NPC24 每日三选二/bonus 的 DB 周期入账；尚未实现。Windows 成功扣费、原 Query、断线至冷登录实机闭环仍开放，其他来源、材料、完整销毁/指标与通知/迁移发布未完成。完整 goal active，默认关闭、K=0.1 与 0.72/1.08 许可保持，不用 Docker、不改远程资产。

## 第九阶段当前审计：原生已报价，未成功收费（2026-10-21）

独立repair原始完整库存source gate已修复，支持真实命名装备位和合法缺省UID，同时保留严格raw presence/dup与actual inventory hash；buy gate逐字不变。Host60的39/39含5项rich-source，正向Quote→合法当前30秒Quoted→单Confirm实际TestSink→Pending→原Query Unknown仍Pending只证明限定host路径，不是GUI或物理WS收费。producer60b为7案例/1测试，UI58为17/17；Runtime50的repair15/邻接41、Gateway46恢复纯5+物理WS1、PG45当前会话/撤销/原有效期1项保留，各轮不累计。

新EXE59普通登录/StartGame/NPC/Repair后，Bag拖入取得服务端费用6；首次显式Close未收费，顶X尝试未建立关闭。第二次报价到OKAY约97秒，超过原30秒TTL，正常拒绝且未发Confirm。停服后独立psql的`C:/mir2-build/hunting-native-pg-final-after-deadline-56.json`仍为0结果/0销毁、绑定1,000,000、旧gold 1234、Bag100/1000、save_version15/revision14。Quoted仍显示通用选择文案是小UX待办。

fixture56已达到2小时截止正常退出0，proxy56按stop marker退出0，EXE59仅自己的窗口Alt+F4正常退出0，其他游戏未操作。原当前窗口30秒快速确认请求已撤回，当前不要操作；下轮新隔离环境再验成功Confirm→Pending Close→原Query→断线→普通保存冷登录，不延TTL/改clock或绕guard。

当前汇总见[阶段九审计](generated/hunting-economy/acceptance-20261021-repair-native-windows.json)。经济公式/PG共同事务/04:00调度/outbox及Grafana的既有有界验收保留，真实来源仍限受限一次性任务、修理与转换费；新钱包partial不能当全服覆盖。其他产出、材料传播、维护/PVP/沙巴克销毁、全服市场画像收益、通知及迁移发布仍开放。完整狩猎经济任务未完成，默认关闭、K=0.1、0.72/1.08许可保持；不提升Crystal/Candidate或生产资格。无Docker/WSL/虚拟机、远程资产修改或其它窗口干扰。以下较早状态及勾选保留历史范围，不据此计算当前完成百分比。

## 第八阶段：浏览器恢复核对与 Windows 原生优先（2026-10-20，历史范围）

浏览器已用本机隔离 Axum/PG 的合成账号完成普通登录/StartGame、真实 NPC 服务、Bag 拖入、服务端报价 6 绑定金币及一次 Confirm。测试 transport 仅省略一次已提交 owner 回执；Pending 主动 Close 后仍可原 Query。独立 psql 在 Query 后、普通 Alt+x 退出后及服务停止后均核对：结果/销毁各 1 条、扣费/销毁 6、绑定余额 999994、旧 gold 1234、Bag 耐久 970/970。该路径不是强制服务端 OutcomeUnknown，本轮未重新登录验证。见 [第八阶段证据](generated/hunting-economy/acceptance-20261020-repair-browser.json)。

实际发现 Query 成功后因 NPC 来源已退休而退回旧空面板，已修为保留原回执的只读完成状态至普通 Close；不会恢复选择/Confirm 权限。钱包另移独立可读区域。最终专项 109/109（UI 20）、原生 Next41 严格生产构建/13 页通过；截图来自修复前的 Next30 浏览器轮次，不能声称这两处修复后的视觉验收已完成。Gateway40 纯 25/25、两个真实 WS+PG 回归 2/2 均通过。

用户明确下一优先项是 Windows 游戏原生客户端。已核实当前 Bevy EXE 源仍旧 raw repair 与旧 gold，本轮 typed hunting 协议/钱包尚未接入；浏览器或 Windows 上原生执行的 Next/Gateway 均不能代表 Windows 游戏通过。1.95.0/MSVC、独立 C target、jobs2、locked/offline 的当前 native baseline check 已通过（42 条现有警告），尚无新 EXE 链接/运行/候选包或 Windows GUI 结论。按 [Windows 接入计划](HUNTING-NATIVE-WINDOWS-INTEGRATION.md) 先约定独立 repair custody/实际 Applied 接口，再接现有原生窗口。

完整 goal active、默认关闭、K=0.1 与 0.72/1.08 许可保持；不运行 Docker/WSL/虚拟机，不修改虚拟化、远程资产或旧 gold 定性，不停止游戏。三个外来 QA JSON、原主树和旧 EXE 保留。本轮不增加 Crystal/Candidate 百分比；后续来源、材料、全服指标、通知和迁移发布仍开放。以下旧段落保留当时范围。

日期：2026-10-19（北京时间）。用户已授权建立持续目标并实施。该系统是本服定制经济规则，不改变或抬高 Crystal 1:1 验收结论。

更新：2026-10-20。用户明确要求本机不用 Docker；开发验收采用原生 PostgreSQL、原生 Grafana，不启动 Docker/WSL/虚拟机。本次启动的 Docker Desktop 进程已退出，服务恢复为停止；未关闭游戏或改动系统虚拟化设置。

当前第七阶段已通过真实原生Axum WebSocket+PG修理、原结果查询、实际保存后冷加载及钱包Origin约束验收，见 [第七阶段证据](generated/hunting-economy/acceptance-20261020-repair-network.json)。第六阶段已接现有Page/NpcRepairService/Shop/Shell、费用/只读钱包和完整snapshot apply；真实浏览器点击及玩家流程仍未验，不新建网页。完整goal active、默认关闭，不代表生产上线、全服资产覆盖或Crystal百分比提升。第四至第六阶段的历史计数、当时范围及失败记录保留在下文，不累加为本轮结果。

## 权威边界

游戏主服务端和 PostgreSQL 事务是资产唯一真值源。Grafana 只读观测，Python 只做对照预演。个人 Session、地图 Zone 和各 Gateway 不能各持有一份独立许可池。正式服、测试服按经济域隔离。

新双钱包与旧 `gold` 分开保存。旧角色退出保存没有新钱包写权；本轮不把旧金币重新定性、不充值许可、不接触远程资产。新业务初始禁用，初始化许可库存为零，只能由已提交的真实销毁在日结中增发。

## 冻结公式

- 一许可等于 1,000,000 原子，允许转换 1,000,000 绑定金币；一金币消耗一原子。
- 新增许可单位数 = 已闭合经济日销毁金币当量 × K / 1,000,000；默认 K = 1/10，对照 K = 1/5。
- 从本次转换毛额扣除 1/50 手续费，最小金币单位向上取整；手续费在同一事务记入日常销毁。
- 日结先增发并应用硬上限，再对新库存执行 1/150 日衰减。整数硬上限截掉的额度不结转，合法分数余数保留。
- 金额、库存、比例和中间乘法采用检查溢出的整数，不使用浮点。
- 经济日固定北京时间 04:00 开始，区间左闭右开。日期由服务端数据库时钟确定；玩家不能上报销毁发生时间或销毁当量。

`config/hunting-economy.json` 是默认禁用政策；Rust/Python 共用 `fixtures/hunting-economy-vectors.json`。配置版本不可变，分母改变时必须明确处理池内旧余数。

## 已确认的参数选择与上线关口

1. 用户已选择保留 K=0.1，重新测算额度。假定日销毁总量为720万金币当量，日增发基准为0.72许可；默认配置改为720,000原子，硬上限1,080,000原子（1.08许可）。旧72/108许可与每日7200万、每月216000万的供应目标撤出默认配置，不能继续作为收益承诺。
2. 若每日总销毁（含转换费）确实恒为720万、结算前许可库存为零且当日用完所有额度，日增发720,000原子、衰减4,800原子，毛转换715,200金币、费用14,304金币、净到账700,896金币；30个同条件经济日毛额21,456,000、净额21,026,880。该数是固定情景，不是已验证的运营稳态：手续费会影响次日销毁；拆单有取整费用；既有许可库存可支持更高单日消费，硬上限只限制新增许可。
3. 每日 0.2/30 衰减的 30 天复利损耗约 18.18%，并非精确 20%。当前实现忠实采用给定日公式。
4. 非绑定金币存量基准未给出，不能拿每日流量当存量告警基准。
5. 材料解绑定需要独立估值版本和手续费来源规则。只记录实际永久删除的资产，掠夺/缴获中的所有权转移不计销毁。
6. 原重度100万/普通20万的个人月上限改为5:1分配比例参考，不构成供应承诺，本轮也没有实施个人配额。个人配额、画像归类和旧资产迁移须在发布前明确；监控画像不能成为客户端自行选择额度的入口。

## 实施队列

- [x] 纯 Rust 定点公式、严格政策解析、04:00 日键与共享案例。
- [x] 同政策/案例的整数 Python 对照模型。
- [x] PostgreSQL 新钱包、全服池、销毁、转换、日结与 outbox 同事务持久化。
- [x] 真实 PostgreSQL 并发、幂等重试、回滚、提交不明与日结追赶测试。
- [x] Gateway Bearer认证HTTP钱包查询与转换接口，事务内核对角色归属、封禁、会话撤销/过期与UUID incarnation；没有原始来源记账接口。
- [x] 普通角色删除与完整账号删除的同事务新钱包保护，非零资产拒删；零余额历史保留与新UUID隔离。
- [x] 无物品一次性金币任务的同事务真实来源：普通FinishQuest/CallNpc处理器、完整个人奖励checkpoint和绑定入账共同提交；独立PG与正常包分发测试通过。
- [x] NPC单物品修理的服务端Quote→Confirm→Query、零费用结果、完整checkpoint/绑定扣款/销毁共同事务与Unknown/断线前恢复；真实本地owner及Shared网关PG验收。
- [x] 服务端同owner回合快照与required nullable snapshot帧、已知提交失败保留及纯Web client adapter；限定模块/原生PG验收通过。
- [ ] 现有Page/NpcRepairService/Shop/Shell的typed报价/确认/查询接线、费用/绑定钱包展示、实际snapshot apply及物理客户端/真实网络保存重登验收；不新建网页。
- [ ] 种植、挖矿、任务及维护/PVP/沙巴克的可信来源接线；绑定身份贯穿交易、邮件、NPC、合成与丢地。
- [x] 进程生命周期的04:00调度、空服运行、重启追赶、锁等待/退出有界；不依赖玩家请求。
- [x] 日结outbox到永久时序版本的幂等导出、提交不明恢复和并发消费；默认仅新钱包partial诊断，ACK与指标/消费回执同事务。
- [ ] 真实成交、存量、画像与收益数据接线，缺失值保持 NULL。
- [x] Grafana只读视图、14个面板、4类业务告警和逐服数据完整性告警配置；实际PostgreSQL规则与只读权限验收。
- [x] 原生Grafana13.2.4导入、verify-full数据库TLS、实际查询/表达式及合成服目标图表浏览器抽查。
- [ ] 全面板布局、真实市场数据与钉钉/企业微信测试通知。
- [ ] 参数与旧资产迁移发布资格、普通玩家保存重登及最终部署验证。

## 第一阶段实际验证（保留历史范围）

隔离工作树分支 `codex/hunting-permit-economy` 从 `2e9d95bbb3cd9f7852980af385a1298b57a1511e` 建立。原工作区的 SLG、运行时和配置未提交修改未复制、未覆盖。

- 共享政策Rust12项/Python14项通过，包含已确认K=0.1的30日情景；K=0.2比较沿用相同1.08许可硬上限，原始1.44许可被截为1.08许可。
- economy-service纯单元9项、真实PostgreSQL核心5项/时序导出10项通过，覆盖库存竞争、同请求重试/冲突、角色代际隔离、来源回滚、COMMIT回执丢失、跨日补结、参数切换保护、事务局部超时复原、指标/ACK共同回滚及完整版本防降级。
- Gateway最终10项（含真实PG4项）通过，实际80个空服历史经济日追赶及80个partial指标导出、日唯一性和持锁退出均通过。早期8项和clock4项与最终执行重叠，不累计为新场景。
- simulation删角6项（纯2/真实PG4）通过：普通删角拒绝非零钱包、内存/DB保持、零钱包可删保留审计历史、新UUID不继承旧钱包及两方向账号锁竞态。新来源仍必须履行同事务账号/角色授权契约。
- 邻接回归：既有删角4项、身份8项通过；身份过滤器带出的2项既有外部PG测试按原条件忽略，未改为通过。原测试场景均保留。
- simulation库及Gateway库/二进制`cargo check --offline`通过。现有simulation/gateway告警保留，未进行无关清理。
- Grafana离线33个合成案例/72项断言/216项配置检查通过；真实PG29个物理schema案例/57个规则期望及历史版本/覆盖资格/只读DML拒绝通过，测试事务整体ROLLBACK。不是Grafana实际导入或通知送达证明。
- 早期失败保留：Rust初次格式差异、SHA2输出格式不兼容、过严的同参数启停保护测试、未应用0017导致首次PG观测测试缺表、新clock测试字段名`inventory_atoms`误写为`stock_atoms`之前的编译失败。均保留诊断并修复后重跑，没有删除原场景或放宽业务阈值。

这些结果覆盖独立的新经济核心，不证明所有实际游戏产出/销毁已经接入，也不证明远程玩家或生产发布。持续目标保持active。

可审查的执行摘要与本地原始日志SHA256见 [验收记录](generated/hunting-economy/acceptance-20261019.json)。最终政策/服务包格式检查、新增Rust文件格式检查及Gateway库/二进制编译检查均通过；未对旧巨型模块做整体格式化。

首阶段本地提交为`99ce96f9fb8d8a6c6207f4f9c88ee7459c98bc10`，63个变更文件均从该提交逐一读取并校验，见 [提交核对](generated/hunting-economy/publication-20261019.json)。提交后的自动GC遇到已有损坏对象`1ae7f886fa87573bdbb59caf0772103929fac94c`导致repack失败，未影响本次提交创建；没有重写、清理或修复无关历史，没有推送或部署。后续提交按单次命令关闭自动GC，不更改仓库配置。

## 第二阶段：首条真实任务来源与原生观测

服务端按真实effective任务定义选择无物品、无carry/item任务、金币大于零且cadence=None的一次性任务；没有为测试账号硬编码任务白名单。已支持原有经验、等级、credit和Guild XP效果，共同写入完整存档CAS与新绑定钱包，不再另发该笔旧gold。带物品/周期任务仍明确拒绝；例如V2首个物品任务尚不支持，不能据此宣称整条新手链已闭合。

新增迁移0021保存独立于定义摘要的角色终身领取槽及不可变完整checkpoint。StartGame首次从canonical角色加载时绑定PG UUID；不能在发奖或旧会话恢复时临时领取当前UUID。全服锁、账户/Guild锁、角色代际、旧存档revision和来源完成状态在同一PG事务核验；重复请求只返回原回执。COMMIT或外层worker结果丢失时保留原金额/摘要/代际和冻结epoch，正常原NPC请求可以恢复，其他任务、NPC、角色或另一冻结不能借此解冻。

冻结归属由同一write-state锁内的Writable→Frozen原子转换确定，不能把已存在冻结的旧理由和新epoch认领为自己的恢复许可。另一个操作先冻结、原请求后进入Unknown，以及反向顺序、相同理由再次冻结，均有明确测试；不属于当前来源的冻结保持拒绝，不能只因找到已提交回执而清除。

规划World的revision、账户镜像、Guild journal和时钟均独立，确认提交前不改变真人ECS。发布只接受存档的直接后继revision，不把更晚完整checkpoint的revision赋给旧个人状态。HP/MP上限按当前有效buff/Guild/新等级重算；Unknown恢复保留当前Zone伤害、耗蓝和死亡，不重放旧血量或复活。缺少持久vital事件顺序时，恢复不补发可能遗漏的一次升级回血；精确重排仍是剩余边界。

当前来源仅支持未复制的本地权威runtime。旧副本协议重放原任务命令会产生legacy/new差异，也不携带原StartGame代际；在完整结果checkpoint及受信witness复制接通前，启用来源与远端Zone Host/Gate15/普通副本重放和晋升的组合必须在执行前拒绝。私有纯规划fork不是副本API。没有把retained bootstrap的相同姓名、索引或相同存档内容当作代际证明。

原生观测已完成：14面板导入、13个数据查询、29个合成服的57项SQL和57项原生reduce/threshold断言通过，含17个NULL；离线33案例/72模型断言/221配置检查通过。发现并修复Grafana接受导入却执行HTTP400的显式`settings.mode=strict`，改用原生默认strict；SQL和业务阈值保持。浏览器实际抽查三层堆叠、许可、玩家饼图、健康值及缺失收益空曲线，未将合成数据当真实运营数据。验收后已停止本次原生Grafana及13个后端子进程，详见 [原生验收](generated/hunting-economy/grafana-acceptance.zh-CN.md)。通知接收、所有面板布局及真实市场数据仍未验收。

最终定向测试51项全部通过：simulation30（来源12、真实任务分发12、删角6），Gateway库20，Zone Host二进制环境门槛1；其中21项实际使用原生隔离PG。Gateway库、主二进制及Zone Host最终编译检查通过，选定28个Rust/Cargo/SQL输入的前后SHA256一致。服务包本轮9单元+5核心PG+10导出PG通过。邻接任务115（默认忽略5）、Guild XP11通过，最终冻结修复后重跑publication10（忽略2，相关PG删角另已实际执行）。过滤器有重叠，不把这些数字相加成独立场景或全workspace结论；编译告警及旧失败日志保留。

任务PG fixture已开启共享Guild权威，但未建立真实Guild成员；实际成员的任务分成/等级变化及同事务Guild写入专项仍需补测，不能以该开关或既有Guild单元回归替代此证明。

原始失败保留：初次编译的私有函数可见性/SHA2 hex格式问题；隔离测试把会推进tick的准备操作误当成prepare的副作用（保留全部隔离断言后修正取样点）；Windows旧测试EXE被DeltaForce占用导致两次LNK1104。没有结束游戏、删除锁定文件或改动保护设置；使用Cargo原生test profile和唯一输出名构建、实际运行同一测试harness。执行记录及最终范围见 [第二阶段验收](generated/hunting-economy/acceptance-20261020-quest.json)；这仍不代表全部生产/销毁来源、材料传播或生产发布完成，goal保持active。

## 第三阶段：真实Guild分支与修理事务基础（历史范围）

以下保留第三阶段当时的交付与验证边界；真实修理来源见第四阶段，当前同步载体与纯client adapter见第五阶段。

补齐真实Guild成员的3项原生PG测试：实际共享成员epoch=2，服务端有效EXP属性404，任务基础48EXP得到241EXP，个人4→5级并保留240EXP，Guild得到2EXP跨越45000升级线；完整奖励、Guild源回执和绑定金币共同提交。覆盖普通LogOut/新配置Login/StartGame读回、提交前全回滚、Unknown后更改当前buff仍只恢复原奖励。没有发现需修改任务业务逻辑的缺陷。

新增正额绑定扣款与面额销毁的共同事务API及0022迁移；授权回调先于原回执查询，来源完整checkpoint、wallet、burn、operation/outbox同事务，重复操作不再执行来源。来源金额只能是实际扣除面额，不能套用材料估值倍率，也不能重复登记转换手续费。原生PG覆盖并发余额竞争、不同角色/UUID、摘要冲突、提交前/提交后回执丢失、禁用/日结欠账后的原查询和局部超时恢复。另在已有旧`bound_credit`及新操作记录的隔离schema中重复执行0022两次，六类经济表完整内容保持一致；验证事务最后回滚。

NPC修理提取纯`RepairPlan`，保留普通/特殊、背包/装备优先顺序、租赁/附加属性定价和递归元数据。旧金币路径仍原样扣款，**绑定钱包修理与真实销毁尚未接通**。私有物品变更见证使用随机会话scope及不可恢复的checked u64；Bevy u32仅感知两个Inventory资源在短借用区间内的实际写入，不作为物品身份或持久授权。它能使同值覆盖、ABA、checkpoint恢复后的旧见证失效，同时保留正常闲置tick。

实际验证：经济服务32项（21项PG），仿真focused57项（33项经济来源含20项PG、6项修理计划、18项变更见证），邻接105项（旧修理10、价格4、Guild经验11、NPC购买80）全部通过，共194项互不重叠的顶层测试，零失败/忽略；这不是全工作区测试。Gateway库、主二进制和ZoneHost编译检查通过，321个选定源码/迁移输入构建前后SHA256相同。初次修理测试误认两种NPC fixture的费率配置，两轮失败日志保留；按真实数据确认普通费率2.0、特殊fixture缺省1.0后，仅修断言，业务未迁就测试。限定格式检查通过。

所有运行均原生、串行Cargo、2个build jobs；继续用唯一测试输出名，未关闭DeltaForce或改动虚拟化。复核时退出昨日遗留的独立`docker-sandbox.exe` CLI；Docker服务仍Stopped/Manual，无Docker进程。三个外来QA JSON保持原字节且未暂存。详见 [第三阶段验收](generated/hunting-economy/acceptance-20261020-repair-foundation.json)。

第三阶段结束时的下一步是带服务端令牌的修理报价/确认/查询、零费用持久结果、完整checkpoint与Unknown恢复；该阶段基础不具备开启真实修理经济或生产发布资格。默认禁用及K=0.1/基准0.72/硬上限1.08许可保持。

## 第四阶段：真实NPC修理来源与typed carrier（历史范围）

以下保留第四阶段当时的来源及326项去重测试（81项真实原生PG）范围；当前快照载体与纯client adapter状态以第五阶段为准。这两个阶段有回归重叠，不相加作为新增场景数。

`runtime/hunting_repair.rs`与`config_hunting_repair.rs`现在执行真实NPC修理：报价来自原有普通/特殊修理定价与完整`RepairPlan`，绑定原角色PG UUID、DB/realm、会话物品变更见证、完整目标和30秒有效期。客户端Quote只提供UID/类型，Confirm/Query只提供服务器生成的随机quote ID，不提供金额或销毁当量。首次确认重新校验真实NPC/范围/类型/bind、物品见证和完整checkpoint CAS；报价过期和NPC条件不阻挡已提交原结果的授权查询。

正额修理在同一PG事务提交完整修理checkpoint、绑定扣款、同面额日常销毁、持久操作结果与outbox；旧`gold`保持。0023增加零费用持久操作，零费用不创建钱包、不扣款、不产生销毁；0024保存报价及原来源结果。成功重放只返回不可变回执，不重装旧物品、不覆盖当前钱包，也不重发历史`ItemRepaired`。新增销毁仍由下一闭合经济日日结生成许可，没有即时补充额度。

Pending/Unknown保留原报价和会话，恢复先查询原结果，不提交替代操作。断线保存前先在真实owner epoch guard内协调原结果；仍未决则Retry，不捕获或journal旧checkpoint。安全投影后再捕获当前Zone transform/vitals与完整存档；不安全或角色生命周期已变化时真实退役旧World、移除Shared presence并清Gateway身份。可信登录刷新可恢复自身原提交的镜像/冻结，保留其它冻结；已有World重复StartGame不能借登录刷新解冻或获得新scope。

独立`huntingRepairOwner` v1 carrier已接严格字节解析、当前认证账号/会话验证、Gateway owner lease、本地InProcess/Shared和Simulation。u64使用规范十进制字符串，合法旧武器槽UID `0`保持；重复/未知字段、客户端价格和旧UID-only确认拒绝。远端/default owner明确unsupported，不扩NPC purchase v1或回退legacy。启用新经济时原`RepairItem`/`SRepairItem`拒绝；未启用仍保留旧修理行为。现有游戏UI与同回合快照同步尚未接入此新carrier。

最终原生验证范围：

- 经济服务（build17）：13纯+27真实PG，共40项通过。
- Sim build21 hunting：29纯+45真实PG通过；邻接修理计划6、物品见证18、旧修理10、价格4、Guild经验11、NPC购买80通过。
- Gateway build23 hunting：19纯+9真实PG通过，其中新增carrier PG 5项实际经过严格wire parser、真实Gateway/InProcess或Shared、真实NPC及数据库；另teardown persistence5、zone drain3、NPC purchase18通过。库、主二进制、ZoneHost检查退出0。
- client-wire（build20，源码未变）：29项通过。`save_recovery`过滤器命中0项，不计测试通过数。以上不是全workspace或浏览器/真实WebSocket断线验收。

保留失败与修复：Gateway19的3项PG夹具遗漏真实routing context和PG恢复目录/MAC，修夹具后再验，未绕过登录保护。Sim21整批曾45通过/29失败，原因是PG连接池在单进程中滞留触及30连接上限；保持`max_connections=30`，45项PG改为逐fresh process全部通过。Gateway22的2项新Shared PG误把Windows正常29字节`.mir2-directory-durability`控制文件计为quarantine；只修测试，对该文件精确校验名称/类型/字节，其它文件和隔离结果仍拒绝，Gateway23全部通过，原失败目录和日志保留。

证据见 [第四阶段验收](generated/hunting-economy/acceptance-20261020-repair-source.json)。没有启动Docker/WSL/虚拟机、关闭游戏或提高PG连接上限。第四阶段结束时下一步是现有修理UI和当前快照同步，再做物理客户端/网络验收；材料、其它来源、复制、全服指标、通知与迁移发布仍开放。默认禁用及K=0.1/基准0.72/硬上限1.08许可不变，完整goal继续active。

## 第五阶段：同回合快照载体与纯Web client adapter

修理与快照在同一真实owner回合及epoch guard内采样，typed返回修理结果和该回合的可选快照；Gateway不从外层个人Session另读快照。wire帧必须包含`snapshot`字段，成功可为真实client-view快照，无法提供时明确为null。Shared尾部失败、采样panic、client-view投影或2MiB上限检查失败均保留已知不可变修理回执，不改写为Unknown、不再次执行来源，也不以旧快照替代。

实际发送helper的RecordingSink3项验证：有snapshot时先发送同回合packets再发修理帧；无snapshot时先发保留回执的修理帧；编码失败零发送。此处使用真实发送helper和记录sink，尚未经过物理WebSocket。realmInfo的`npcRepairMode`按realm静态绑定为`hunting-v1`，不会随政策enabled切换回legacy，避免禁用时绕回旧金币修理。

`apps/web/lib/hunting-repair-client.ts`是纯adapter：严格解析原始JSON、规范十进制u64、nullable snapshot及拒绝码；固定物理owner、角色scope和mode，使用同owner不复用的request ID并发布不可变view。Confirm在尝试发送前即进入Pending，发送异常、超时或缺少/应用失败的snapshot都保留原quote，只能Query；关窗或resetView不丢Pending。已知回执不会被后到NotSubmitted抹除，reloadRequired退役且不应用snapshot；跨owner/角色迟到、重复字段及回调重入拒绝。回调后再次校验control和active token，防止回调内超时清理后继续发送或应用旧帧。

Root最终26验收：

- Gateway：25项hunting纯测试、10项真实原生PG（其中6项实际carrier PG），邻接teardown persistence5、zone drain3、NPC purchase21、realm2；共66次执行，按完整测试名去重62项。三个dispatch测试和一个realm测试重复执行不重复计数。
- client-wire：33项通过。Web adapter：独立Node42/42、模块strict TypeScript及语法检查通过。
- 本轮合计137项去重测试、141次执行，其中10项真实原生PG；Gateway库、主二进制、ZoneHost检查及限定新增文件格式检查退出0。该数字不与第四阶段326累加，不是全workspace或全前端编译。

证据见 [第五阶段验收](generated/hunting-economy/acceptance-20261020-repair-sync.json)。当前没有接入Page、NpcRepairService、Shop或Shell；snapshot apply只是adapter回调契约，尚未连到真实游戏状态。下一队列在现有修理窗口接typed报价/确认/查询、费用与绑定钱包展示、真实snapshot apply，再验物理玩家确认/拒绝、Pending恢复、断线保存重登，不新建网页。原生运行、PG连接上限30、串行Cargo jobs2及三个外来QA JSON保持；未启动Docker/WSL/虚拟机、未改远程资产，默认关闭、完整goal active，全服覆盖与生产资格仍未取得。

## 第六阶段：现有游戏修理窗口与新钱包观测

Page、NpcRepairService、Shop和Shell现已接独立typed Quote→Confirm→原Query。realm握手按物理socket/generation固定模式，缺握手拒绝修理，冲突声明持续封闭；hunting-v1不会在政策禁用或失败时回退legacy。每个adapter持有冻结actor，实际message闭包在其它接收回调前捕获原binding；未知/畸形/不配对的修理namespace终止，不交给泛化旧处理或新的current adapter。

新费用只显示服务器报价中的规范十进制绑定金币，不用旧gold推断可付金额。新钱包GET是观测，不能授权Confirm；角色索引由冻结actor确定，Bearer只在同源请求headers里，identity/socket/actor替换、过期、旧请求与revision倒退均拒绝。固定代理只连接可信配置上游，16KiB/8秒限制、禁止跟随重定向和转发cookie，所有响应no-store；没有新资产写接口。

有效选择/完整物品来源见证只申请Quote，已报价后才Confirm；Confirm发出后不论send异常、超时或缺少/应用失败的snapshot，只能Query原quote。Hold在新模式停用。已知结果携带的完整快照连接现有真实Quest/Spells/Mail/World/Inventory apply，不伪造NPC purchase Producer/Core Applied或Hero证明；原actor和完整快照确实应用后才解除未决。Query独立于物品目标、NPC和新资产操作就绪；主动关闭会撤销选择/NPC权限，但保留只读recoveryOnly面板，避免NPC退场后原查询无入口。非Pending关闭正常隐藏。

最终29专项101/101（纯client42、UI16、实际Page函数15、wallet/proxy28），原生Next16.2.11生产构建包含严格TypeScript与13静态页；另使用真实TS5.9.3全配置严格检查315 roots/982 source files、零错误。旧NPC/Bag/Storage/Stage5八份脚本统一353/353顶层测试通过，其中Stage5文件级结果内部执行506组自定义检查；不把文件级包装与内部组数重复相加。旧夹具补入真实依赖/空custody与显式legacy模式；原场景保留，Confirm旧表达式改为完整32组合真值表及新增bound拒绝检查。编译后的钱包路由另有10项本机物理HTTP验收：请求形状/身份拒绝、完整十进制、固定GET、禁止POST/重定向/206/超限及错误保留；上游明确为合成只读数据，没有游戏资产变动。以上数字不与历史137项或PG测试相加。

证据见 [第六阶段验收](generated/hunting-economy/acceptance-20261020-repair-ui.json)。首轮旧夹具缺新增依赖、Page generic-event类型错误及Windows跨磁盘Next入口失败均保留历史日志；类型问题已修，Next包在独立C盘逐文件与原依赖校验一致后原生构建，未安装包或修改主工作区依赖。原tsconfig/编译器和三个外来QA JSON均保持。没有浏览器截图、物理游戏WebSocket或玩家保存重登结论；下一队列是原生隔离环境真实客户端流程，其它来源/材料/全服指标/通知/迁移发布仍开放。默认关闭、K=0.1、0.72/1.08许可和goal active保持；Docker停止/手动、相关进程0，不运行Docker/WSL/虚拟机、不改虚拟化或停止游戏。

## 第七阶段：真实物理WebSocket、PG冷加载与钱包Origin

独立`web_hunting_repair_network_tests.rs`使用真实Axum loopback WebSocket、PG身份会话和Shared Zone factory，没有模拟WorldRuntime。普通NewAccount/Login/NewCharacter之后，仅在进入World前离线准备一个损坏的真实目录WoodenSword Bag物品（完整实例metadata、安全UID）和合成绑定余额/旧金币；不把这些准备当作玩家正常产出。之后通过普通StartGame、实际NPC对话中的服务端`@repair`链接和`NPCRepair`进入服务，使用原始wire字节执行Quote/Confirm/Query，验证原ItemRepaired先于同回合完整snapshot、绑定扣款/销毁/结果/完整checkpoint共同提交、旧gold不变以及原Query不二扣或重播物品delta。

第一用例还验证未认证和无对话报价拒绝、损坏物品上的旧raw repair拒绝，以及政策关闭后仍声明`hunting-v1`并允许查询原提交回执。政策关闭的新报价在当前runtime返回有界`Unavailable`，不能写成已经验证精确`Disabled`错误码。第二用例发送Confirm后不消费任何socket回执，由独立PG连接先证明真实提交，再drop连接并等待服务端真实保存/retained handoff；测试受控移除该已保存旧owner及其路由并丢弃旧runtime后，重建全新config、account mirror、registry/cache/identity/WebState，再普通Login/StartGame冷加载并Query原结果。该路径不证明浏览器resume，也没有强制注入服务端OutcomeUnknown。

同一真实PG身份和HTTP listener验证只读钱包：没有Origin返回403、配置Origin与有效Bearer返回实际新钱包、坏token返回401。Next代理另修复公开Origin传递：`MIR2_WEB_PUBLIC_ORIGIN`只接受可信单一HTTP(S) origin（可带根`/`，不允许凭证、路径、query或fragment）；未设置时取`request.url.origin`。如果请求带Origin，必须与该canonical origin精确匹配；不使用Host、forwarded或cookie建立权限，原Gateway allowlist/Bearer校验不放松。真实Next运行呈现内部request URL，部署时应按`apps/web/.env.example`显式配置公开origin并与Gateway的`MIR2_ALLOWED_WEB_ORIGINS`一致。

最终37：两个真实WS+PG用例逐用例fresh process运行，2/2通过；hunting纯25/25、Gateway lib+bins检查及仅新文件rustfmt检查通过。Web专项105/105（含wallet/proxy32/32），原生Next30生产构建包含严格TypeScript和13页，编译后合成allowlist上游HTTP12/12通过。真实PG钱包接口与合成Next代理HTTP分开记载，不把合成上游当作游戏资产，也不与此前阶段的计数相加。

首次原生编译后的夹具失败和修正保留在 [本阶段证据](generated/hunting-economy/acceptance-20261020-repair-network.json)：恢复MAC key改用满足原diversity guard的多样测试值；完整bootstrap读取预算由512帧改为16K帧/32MiB/10秒，并只统计帧类型，不输出身份或完整payload。高层Interact实际返回ObjectChat与snapshot中的dialog，不能假定NPCResponse；修理链接从真实服务端dialog读取。旧raw repair拒绝以同FIFO队列的后续Quote作为barrier，KeepAlive走reader fast lane，不能证明前一个命令已执行。这些修正没有放宽生产业务、Origin、密钥或资产检查。

本轮没有真实浏览器DOM、截图、正常玩家点击、浏览器Pending恢复或完整新手流程验收。下一步在既有游戏窗口完成这些确认/拒绝、原Query和保存重登，再继续其它来源、材料绑定传播、全服指标、通知及迁移发布。默认禁用、K=0.1、0.72/1.08许可、旧gold不重分类、远程资产不变和完整goal active保持。只用原生环境，PG上限30、Cargo串行jobs2，不运行Docker/WSL/虚拟机或关闭游戏。

## 后端接线与交付物

- 实现：`packages/economy-policy`、`packages/economy-service`；迁移从0016开始，0015保留给其他工作。
- 玩家接口：`GET /v1/economy/hunting-wallet?characterIndex=0`；`POST /v1/economy/convert`，请求为`{"characterIndex":0,"grossGold":"1000000","requestId":"一次请求的稳定ID"}`。必须复用同一请求ID与金额恢复不确定提交；账号、经济服、政策、销毁量和角色代际全部由服务端确定。金额响应为十进制字符串。
- 启动开关：`MIR2_HUNTING_ECONOMY_ENABLED`默认false；启用需显式`MIR2_REALM_ID`及账号PostgreSQL SourceOfTruth。可用`MIR2_HUNTING_ECONOMY_POLICY_PATH`初始化新零库存经济服，既有库的政策版本不同则拒绝静默切换。要开放玩家操作，还须数据库中当前政策的`enabled=true`。
- 日结线程每15秒检查数据库经济日，每批最多31天；迟于04:00的扫描只关闭已经结束的日，无需在线玩家。独占短期PG连接设置连接5秒、锁2秒、语句10秒超时；进程关闭最多等待worker12秒，数据库损坏/断网下可留待重启恢复。没有对共享登录/存档连接发送取消请求。
- `infra/economy-observability/README.zh-CN.md`为Grafana配置清单；`CONTRACT.zh-CN.md`为指标、告警和完整覆盖语义。新钱包求和仅显示为诊断，价格/画像/收益未接来源时保持NULL，不能冒称全服资产覆盖。
- `infra/economy-observability/EXPORT.zh-CN.md`说明日结outbox可靠导出；每批最多128事件，独立连接与错误处理，指标故障不会阻止游戏启动或下轮结算。只有已写入永久指标和幂等消费回执，才在同事务ACK对应日结事件。
- 真实来源、材料绑定及存档桥接待办见`HUNTING-SOURCE-INTEGRATION.md`。旧金币没有重分类；不能在legacy奖励完成后再另写一次新钱包。

## 每日结算伪代码（与当前实现一致）

```text
进程启动后持续运行；每15秒检查，使用专用数据库连接：
  BEGIN，设置事务局部锁/语句超时
  锁定经济服共享advisory lock
  此后读取数据库时钟，确定北京时间04:00经济日
  读取全局池与不可变政策，锁定池行
  最多处理31个已闭合、未结算的连续日：
    校验日键 >= genesis，且该日没有超前于cursor的历史结算
    分层汇总该日已提交的真实销毁；汇总成功转换的许可消耗/毛额/净流入
    原始新增原子 = floor((销毁金币当量 × K分子 + 余数) / K分母)
    今日新增原子 = min(原始新增原子, 基准日增发 × 硬上限倍率)
    丢弃超过硬上限的整原子，保留合法分数余数
    衰减前库存 = 原库存 + 今日新增原子
    衰减原子 = floor((衰减前库存 × 日衰减分子 + 余数) / 日衰减分母)
    更新库存、余数、revision与连续结算cursor
    插入唯一(realm, day)结算回执与同事务outbox
  COMMIT；不确定提交保留原cursor/请求恢复语义，不能补偿或再次造额度
  独立只读聚合消费已提交outbox，持久写时序版本成功后才ACK
  市场、画像与全服资产未接全时保留NULL/partial，禁止补零
```

库存和所有新来源变更均走相同经济服锁，读时钟在锁后。跨04:00等待的操作属于新日；业务遇到漏结须先完成补结再尝试原请求，不能把事件写入已关闭日期。

## 绑定金币转换伪代码（与当前实现一致）

```text
校验请求形状、规范整数金额、请求ID、Bearer签名与Origin
限制并发数据库请求，进入游戏Gateway的blocking任务
BEGIN，设置事务局部超时，锁经济服，再读取DB时钟
同事务锁账号、identity session、角色：重新验证封禁/撤销/过期和归属
从角色表取UUID incarnation；不能相信客户端账号、服或角色代际
若原请求回执已存在：校验请求摘要与历史摘要，返回原回执
读取并锁政策、许可池与角色钱包
要求政策启用、结算cursor已追赶；绑定金币与许可原子均足够
手续费 = ceil(毛金币 × 2 / 100)
绑定金币 -= 毛金币；许可原子 -= 毛金币
非绑定金币 += 毛金币 - 手续费
同事务登记手续费日常销毁、转换记录、幂等回执与outbox
COMMIT后返回；许可不足返回“当前全服变现额度不足，请等待。”
COMMIT结果不明返回outcomeUnknown，只能用相同请求ID/金额重试，不退款
```

材料解绑定没有在此金币接口中开放。材料需要来源绑定标识、估值版本、同事务扣物与费用来源等完整规则后，才能增加独立服务端接口。
