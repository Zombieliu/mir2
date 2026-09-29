# 整个游戏完整度审计：Crystal → mir2-web3
日期：2026-09-08（Asia/Shanghai）  
范围：整个游戏与交付链，包括服务端、共享世界、Web、Windows、导入内容、存档经济、安全运营、容量与发布。  
性质：当前工作区源码审计、资源清单复算、历史证据复核；不是本次全量实机验收，也没有修改游戏实现或运行破坏性资产用例。

**结论：基础游戏闭环已有实质实现，世界数据导入广，联网与恢复也有真实历史成果；整体仍处于多系统整合与完整玩法补齐阶段，尚未达到全游戏功能冻结。剩余工作包括真正的玩法缺失和资产正确性缺陷，远不只是 Windows 动画、UI 润色或最后一次人工验收。**

**不能给可信的全游戏剩余百分比。** 当前语义分母未闭合；“有数据”“有包编号”“有函数”“测试通过”“正常玩家能使用”“Crystal 一致”是六件不同的事。既有 100% Candidate、约 90%、后端约 49%（45–54%）以及前端 90.5%/74% 等数字，不具备本轮统一且闭合的语义分母，本报告均不沿用。[S01][S02][S38]

本轮审查 36 个系统领域，整理 12 组优先发现。**36 是报告分类数，12 是本轮问题组数，不是整个游戏的功能分母或全部缺陷总数。** 未将“没有全域通过证据”误写成“整个系统没有实现”。

**版本与证据边界**

- 实现基线 HEAD：`58eab9a4a6684b2d2d11d2c533049b64995e7458`，分支 `codex/windows-player-journey`；审计开始有 76 条 Git 状态记录，其中目录项不等于文件数。此工作区含未提交改动。
- Crystal C# 参照：`92b4ce4ab488b11e65f63d3ad22de2e1f25ec08d`；本轮检查 `Client/Server/Shared` 范围为干净状态。导入 JSON 的源数据日期分别来自 4–8 月，重新读到旧 JSON 不代表已用今天的 DB 重新导入。
- 最新开发 EXE：`C:/mir2-ground-label-20260908/mir2-platform-windows-entry-fix.exe`，记录 SHA-256 为 `082D5C6229931BAFCFB84BC8E679EE9F3430F5754A1E255D801E98EBAD1D1B85`。包含相机与重复 StartGame 修复；其文档仍标记 live pending。[S19]
- Windows 558 测试与 Gateway 696 通过、1 个 PostgreSQL 环境忽略是前序修复日志，非本轮重跑；它们不能作为当前发布包、全部游戏或真人完整旅程的通过证明。
- 旧 Candidate、最新开发 EXE、当前 dirty source、7 月容量镜像是不同对象。不得合并成一个“最新版全部通过”。
- 具体源文件、JSON、测试日志和实盘包文件的 SHA-256 见配套目录 [source-snapshot.json](generated/whole-game-audit-20260908/source-snapshot.json)；事实、领域矩阵分别见 [audit.json](generated/whole-game-audit-20260908/audit.json) 和 [domains.csv](generated/whole-game-audit-20260908/domains.csv)。

**可以确认已经积累的能力**

项目有五职业创建、升级、战斗、技能、Buff、背包装备、NPC/任务、商店、死亡掉落、共享移动/AOI/聊天及部分交易闭环。Zone 已有原生怪物攻击、掉落、尸体与墙钟刷新；坐骑/SwiftFeet 三格跑也已有实现。不能再沿用“完全没有共享战斗”“刷新仍全靠个人会话”“三格跑没做”的旧结论。[S02][S05][S06]

物品掉落的完整 UserItem、claim ticket 和持久结算 Slice A–D 已在 8 月 25–26 日完成限定范围修复，恢复日志亦存在。本轮没有证据将 8 月 24 日旧的整个掉落 UID/save P0 原封不动判为当前未修；本次新发现的市场、精炼丢实例是另外的入口。[S02][S03][S23]

联网底座也不是空壳：7 月 25 日保存的 Gate 18 有 500 个不同玩家、120 Zone、30 分钟混合命令；Gate 19 有 500 人一小时及故障恢复报告。它们证明当时镜像、硬件与已覆盖行为的能力，不证明今天全部玩法完整。[S28][S29]

**内容规模：导入清单，不是完成数**

| 类别 | 本轮复算当前文件中的规模 | 含义与边界 |
|---|---:|---|
| 地图 | 464 DB 记录，463 非空地图名 | 包含一个空 DB 占位；不等于 Windows 全图显示通过 |
| 地图移动连接 | 1,999 行 | 不等于全部可达性、脚本门或跨图路线验收 |
| 刷怪 | 6,341 规则，76,181 配置槽位 | 实际可放置与激活数量仍受地图、碰撞、规则影响 |
| 怪物模板 | 555；116 个 AI 值；296 个 image 值 | 模板、AI、图像是不同分母 |
| 职业 / 技能 / Buff | 5 / 109 / 59 | 定义存在，不代表全部效果、失败、时序及视觉 |
| 物品 / 配方 / 商城条目 | 1,628 / 79 / 105 | 实例保真和操作入口另验 |
| 任务 / NPC | 154 / 375 | 不是已实机完成任务数 |
| NPC 脚本 / labels | 634 / 5,709 | 不是完整语言或全部执行分支覆盖 |
| 掉落表 / 条目 | 1,640 / 70,542 | 掉落概率、归属和保存另验 |
| 动作元数据 | 703 库、3,643 动作 | 其中 Monster 421、NPC 236 库；元数据不保证像素可用 |

数据来自 `packages/game-data/data/generated/crystal_*_manifest.json` 与 `apps/web/public/original-ui/frame-sets.generated.json`。具体数组、生成时间、来源哈希和计算式在机器记录中。配置 `crystal_full` 只是启用全部导入世界并不套 platinum 限制；这个名称不承诺全技能、全 AI、全 UI 或全部素材已完成。[S46]

**36 个领域的实际状态**

状态中的“实缺”表示发现了未接入、不可操作或与原版不同的明确路径；“待核验”表示有实现与有限证据，但没有全范围验收。P0 资产正确性指普通功能可能改变或损失物品；P0 体验门指当前基础旅程尚不能视为稳定通过；P1 表示全游戏完整度的重要缺口。优先级不是漏洞等级认证。

| ID | 领域 | 已确认能力 | 剩余差距 | 状态 |
|---|---|---|---|---|
| G01 | 账号、选角、登录重连 | Web/Windows 有完整壳层与服务端鉴权；最新重复 StartGame 修复已构建 | 最新 Windows 登录→进图→退出→重进仍缺实机复验；不能把修复记录当通过 [S19][S24][S25] | P0-体验门 |
| G02 | 移动、奔跑、寻路、相机 | Zone 意图、速度、碰撞、AOI；客户端预测及真实时钟修复 | 用户拉回/相机漂移为有效失败观察；需同版长按、高延迟、拥挤碰撞连续验证 [S20][S21][S05] | P0-体验门 |
| G03 | 地图、建筑、跨图、小地图 | 463 非空地图名与移动数据已导入；两端有地图 UI | Windows 地图素材未全量闭合，keyed 缺源仍允许；跨图旧失败未正式关闭 [S12][S13][S18][S47][S48] | P1-实缺 |
| G04 | 地图规则、门、危险地面、活动 | 环境参数、危险地面、6 个类型化地图坐标事件已有实现 | 18 个通用事件只导入未执行；完整门/墙/活动状态机及时间边界待覆盖 [S10][S02] | P1-实缺 |
| G05 | 五职业、经验、等级成长 | 5 职业及经验升级、基础属性、角色创建链路存在 | 完整等级曲线、职业限制、技能学习/升级与长期成长旅程无统一对照证据 [S02][S05] | P1-待核验 |
| G06 | 技能、法术、资源消耗 | 109 技能模板；个人与 Zone 有多类技能、治疗、召唤实现 | 技能×等级×目标×成功/失败×时序×视觉完整矩阵缺失；两执行路径需分别核对 [S02][S05] | P1-部分 |
| G07 | Buff、毒、控制、状态恢复 | 59 Buff 定义；Zone 有中毒、控制、治疗、地面法术 | 叠加、驱散、免疫、死亡/跨图/断线与剩余时间完整语义待验 [S02][S05] | P1-待核验 |
| G08 | 普攻、远程、命中与受击 | 共享伤害、攻击模式、安全区、动作回包及血条已接入 | 自身攻击 ID 仅修选定回包；异步受击身份及全装备方向动画仍待闭合 [S05][S22] | P1-部分 |
| G09 | 普通怪物 AI | 个人模拟有大量专属处理；共享 Zone 有追击、攻击、控制、召唤 | 共享路径没有整体使用个人专属 AI，不能把个人测试扩展成线上行为完成 [S05][S06] | P1-实缺 |
| G10 | Boss、特殊怪、阶段机制 | 个人沃玛教主等专用状态机存在 | 沃玛教主被围传送/分阶段狂暴未见共享 tick 接入；Boss 全目录待迁入/验证 [S05][S07][S08] | P1-实缺 |
| G11 | 死亡、尸体、收割、复活刷新 | Zone 已拥有墙钟刷新、尸体/收割门及 checkpoint | 全怪/全地图/归属与恢复组合未逐一对照；不能沿用旧“尚无共享刷新”结论 [S02][S05][S06] | P1-待核验 |
| G12 | 掉落、抢拾、归属、奖励 | 完整 UserItem、claim ticket、Slice A–D 原子结算已有边界证据 | 不能外推全部掉落生产者、死亡规则与当前部署崩溃闭环；需保留先前修复成果 [S02][S23] | P1-待核验 |
| G13 | 背包、药品、装备、仓库 | 常规 use/equip/remove/drop/delete/split/merge 与保存存在 | Windows 原版拖拽/双击/锁定仍有替代操作；全容器、腰带优先规则未闭合 [S16][S18] | P1-部分 |
| G14 | 实例词条、耐久、绑定、嵌套物品 | 核心数据可保存丰富实例与图标信息 | 跨市场/精炼出现降为模板再生成的实际缺陷，不能声明全系统物品保真 [S03][S04] | P0-资产正确性 |
| G15 | 任务、任务道具、奖励、成长线 | 154 任务、个人领取/交付/选择奖励与新手多段测试存在 | 全任务分支、日常/限制、职业全旅程、失败/重登待逐项验证 [S02][S09] | P1-部分 |
| G16 | NPC 脚本语言与世界服务 | 634 脚本已导入；大量条件/动作有解释器 | 实例移动、邮件构造、定时器、移除技能/Buff 等源语法未在对应分派覆盖 [S09][S41] | P1-实缺 |
| G17 | NPC 商店、修理、商城、个人仓库 | 两端已有窗口、购买/售卖/修理/回执与部分权限实现 | 服务距离、断线、容量、重复回包、过期 NPC 及全商品限制待整体验证 [S16][S17][S18] | P1-待核验 |
| G18 | 制作、配方、觉醒、镶嵌等加工 | 79 配方及后端多类加工 handler；Web 接收结果 | Windows 缺完整发起入口；Web 部分结果只记日志，不能据此算操作闭环 [S03][S16][S17] | P1-实缺 |
| G19 | 武器精炼与材料价值 | 有存入/取回/取消/概率模型 | 完整物品被存成 key，取回固定数量 1；材料耐久/词条按模板代替 [S03] | P0-资产正确性 |
| G20 | 采矿、钓鱼、采集 | 个人采矿/钓鱼逻辑及部分测试存在 | 全世界 MineZones 为空，矿区分布未导入；Windows 钓鱼菜单禁用 [S04][S14][S44] | P1-实缺 |
| G21 | 双人交易 | 共享邀请/配对、金币即时保管、一次性完成与恢复已改进 | 双方物品格只读；精确物品保管、编辑解锁、容量失败保留报价未完成 [S02][S15][S40] | P1-实缺 |
| G22 | 市场、拍卖、寄售 | 普通包有搜索/购买/取回；Web 有部分入口 | 个人 Stage5 列表、完整物品丢失、固定价语义；完整全服市场/竞价待实现 [S03][S04][S17][S35] | P0-资产正确性 |
| G23 | 邮件、租借、异步资产交付 | 邮件载体、领取、共享租借入口及部分幂等机制存在 | 全在线/离线/跨区/到期/重放/满包组合与 Native 操作入口未闭合 [S02][S16] | P1-部分 |
| G24 | 组队、聊天、协作奖励 | 真实共享聊天、组队归属与奖励路径，历史两人远程证据 | 全组队邀请/退出/跨图/掉线/队长变更及多人连续体验仍待验证 [S02][S28][S29] | P1-待核验 |
| G25 | 行会、行会仓库、权限 | 有成员/公告/权限/仓库/聊天的状态与 packet handler | 普通改成员/接受邀请仍修改个人 guild；完整共享行会权威与跨成员一致性未找到 [S03][S04] | P1-实缺 |
| G26 | PK、行会战、领地、攻城 | Zone 玩家伤害/安全区/模式已实现；本地领地字段与操作存在 | 完整战争调度、城防、积分、胜负结算共享状态机未找到 [S05][S36][S37] | P1-实缺 |
| G27 | 好友、黑名单、师徒、婚姻、排行 | Web 多数有真实窗口及发包回调；部分后端状态存在 | Windows 对应菜单禁用；全服在线名单、关系生命周期还需跨账号验证 [S14][S16][S17] | P1-实缺 |
| G28 | 英雄、战斗随从、召唤 | 个人英雄装备/技能/AI；Zone 部分召唤物有战斗 | 共享个人 tick 禁英雄战斗且未找到 Zone 对应英雄战斗，不能视为在线英雄完成 [S05][S06][S04] | P1-实缺 |
| G29 | 智能宠物、坐骑、跟随与拾取 | 部分拾取/维护、坐骑三格跑及骑乘呈现存在 | Windows 智能生物/坐骑入口禁用；完整控制、装备、跨图、死亡恢复未闭合 [S05][S14][S16] | P1-实缺 |
| G30 | HUD、窗口、名字、提示、键位、设置 | 大量原版位图、背包/任务/地图/设置窗口已实现 | 33 项用户观察清单仍保留；8 项菜单禁用；音量条等交互仍为替代行为 [S14][S18] | P1-实缺 |
| G31 | 人物、怪物、NPC 动作与特效 | 703 库动作元数据；Native 有组合图层及多种效果代码 | 实体图集仅 8 个 Monster 库；其他非玩家帧不走原图 fallback；缺动作降级 [S11][S43][S45] | P1-实缺 |
| G32 | 光照、遮挡、环境动画、声音 | Native 有光照/环境帧与选定精确音效；Web 有独立模块 | 黑色光照反馈未关闭；声音包仅 49 文件；全图/全动作声画时序及真实 DPI 待验 [S18][S21][S49] | P1-部分 |
| G33 | 存档、数据库、恢复、迁移 | Argon2id、PostgreSQL、事务/outbox、恢复日志、旧版迁移与故障测试存在 | 历史成果须保留；当前全部经济生产者及最新版本故障/备份恢复证据仍未统一 [S02][S23][S25][S28][S29] | P1-待核验 |
| G34 | 安全、GM、权限、审计 | 生产 debug 命令拒绝、账号生命周期鉴权、管理认证/限流、Passkey fail-closed | 本次仅抽查关键边界，不能宣称安全漏洞清零；运营全流程/配置与依赖安全需专项门 [S24][S25][S26][S27] | P1-待核验 |
| G35 | 多人容量、HA、监控运维 | 历史 500 人混合玩法与故障恢复通过；有遥测、队列、部署/验收脚本 | 1000 人报告延迟/吞吐失败；3000 人缺合格硬件/最终报告；长期耐久与当前重验待做 [S28][S29][S30][S31][S42] | P1-实缺/验收 |
| G36 | 打包、更新、兼容性、发布验收 | 有 CI、Windows 包及多次可运行开发 EXE | 最新修复包与历史 Candidate 非同一版本；正式签名/资源授权/同状态原版对比/DPI/soak/人工门开放 [S18][S19][S33] | P1-待核验 |

**应优先处理的源码发现**

1. **F01 / P0：寄售会丢失真实物品与堆叠数量。** `packets.rs:3872` 的普通寄售路径移除完整背包 ItemState，却只在 Stage5AuctionListing 中留下 `item_key`；购买与取回重新创建数量 1 的模板物品。实例 UID、耐久、词条、绑定或嵌套状态无法从这个结构恢复。Crystal 把真实 UserItem 放进 AuctionInfo。本轮为静态链路确认，没有在用户资产上执行复现。关闭条件是普通两账号的寄售/购买/取消/保存重登全字段与数量守恒，并使用真正共享的市场权威。[S03][S04][S35]

2. **F02 / P0：精炼也把实例降成模板。** `packets.rs:4122` 存入移除整件物品，槽里只存 key；取回和取消重建数量 1。精炼公式注释明确以模板满耐久和模板属性代替材料实例。这不仅影响归还保真，也会改变矿石/材料价值及结果。关闭条件是实际材料参与公式，取消、满包、失败、重试均不丢实例。[S03]

3. **F03 / P1：共享 Boss 和英雄战斗缺少完整执行器。** 当前个人模拟有沃玛教主的被围传送、HP 阶段狂暴等；共享 Zone tick 没有这套状态机。共享模式又明确排除了个人怪物 AI 与英雄战斗，不能期待它们自动补上。需要逐家族绑定真实 Zone 执行器，验证两位观察者看到同一阶段、伤害、死亡与恢复；不能把共同追击攻击算成全 Boss 完成。[S05][S06][S07][S08]

4. **F04 / P1：行会、攻城等仍存在个人状态实现。** guild、auction、conquest、hero 仍在个人 Stage5SystemsState 内；普通行会邀请/成员处理修改自身资源，未找到完整共享行会权威。攻城主要改战争列表与城主字段，未找到 Crystal 周期、积分、城防、终局结算的完整共享状态机。已有共享交易/组队聊天不能替代这些系统。生产路径拒绝 Stage5Command，所以 QA 的 `conquest.start` 等也不能计作玩家入口；这不是“公网任意执行 QA”的新漏洞指控。[S03][S04][S24][S36][S37]

5. **F05 / P1：Windows 素材覆盖是功能性阻塞。** 实体 atlas 只有一个 starter、7 页、10,482 rects，Monster 仅 000/003/004/005/007/010/012/139 八个库。未入图集帧仅允许玩家族和 DNItems 单帧回退，其他 Monster/NPC/Pet/Gate 缺帧直接不产生图层。当前实包 keyed 地图只声明 0、0141，并记录 2,969 个缺源；普通地图 atlas 仅 13 库/2,305 源帧/57 页。素材有跨图复用，不能说“只能玩两图”；但全 463 地图、全怪物的必要帧远未证明闭合。缺动作还可能退为 Attack1/Standing，动画存在不等于动作正确。[S11][S12][S13][S43][S45][S47][S48]

6. **F06 / P1：脚本语言、事件、矿区有真实遗漏。** Crystal parser 有 50 条件和 99 动作字面量；当前 Rust 对应分派同名匹配为 35/50，需继续对别名与其它解释器逐项归类，不能算 85/149 的完成率。实例移动、脚本邮件构造、定时器、移除技能/Buff 等未在对应解释器分派覆盖；未知条件 false、未知动作记录后继续，可能把任务/NPC 分支静默变成不同玩法。18 个 general event 明确仅导入源数据；全世界 mine_zones 为空，导入器读过 MineZones 后丢弃。[S04][S09][S10][S41][S44]

7. **F07 / P1：Windows 有真实缺失的入口。** 键位、排行、智能生物、坐骑、钓鱼、好友、师徒、婚姻八项原版菜单明确禁用。交易虽有 Deposit/Retrieve 序列化与格子图片，却没有 TradeCell 的运行时点击/拖拽消费者，物品格仍只读。最新金币保管修复并没有关闭物品编辑、解锁、容量失败保留报价等后续工作。[S14][S15][S16][S40]

8. **F08 / P1：Web 也不能作为完整原版的替代验收目标。** Web 已实挂好友/关系/排行等窗口，确实比 Windows 操作面广；但 market 没传 onBid/onList，conquest 只有数据，trade 没有物品编辑回调，部分 craft/refine/awakening 结果只写日志。应追踪“用户输入→普通协议→共享/个人权威→回执→画面→保存”的完整链。[S17]

9. **F09 / P0 体验门：最近基础问题仍需同版实机复验。** 重复进图卡 Starting、持续奔跑拉回、相机偏离、挥砍缺失都有针对性修复和测试，但最新记录仍 live pending。黑色光照与相机是不同问题，不能由相机回归通过一并关闭。33 项清单含已部分实现、仍待图像/输入验证的项，“保留 33 项”并不表示 33 个全未写代码。[S18][S19][S20][S21][S22]

10. **F10–F12：随机/时序、规模、发布证据仍有独立缺口。** Zone 有由时间/actor/rule/salt 推导的确定随机辅助函数，不能自动声称与 Crystal 随机流、抽样顺序和边界一致；相同地，历史容量通过不等于新版本通过，语义分类和统一发布证据仍需闭合。[S01][S05][S28][S29][S30][S31][S33]

**服务端、安全和运维：保留已完成成果，明确上限**

| 证据 | 原始结果 | 本轮解释 |
|---|---|---|
| Gate 18（2026-07-25） | 500 人、120 Zone、1,800.405 秒；1,331,915/1,331,916 命令完成；p95 343.37 ms | 历史混合玩法与结算证据；有 1 次语义失败，错误率仍在该门限内；不是零错误或更高阶段延迟通过 |
| Gate 19（2026-07-25） | 500 人、3,600.294 秒；2,642,218/2,642,287 命令完成；p95 185.67 ms；6 类单故障 | 历史 HA 证据，不应再说项目无跨进程/数据库恢复基础；不自动覆盖后来的完整玩法与版本 |
| Gate 20-load（2026-07-25） | 1,000 人；901.053 秒；success=false；p95 278.00 ms；工作负载命令率覆盖 59.39% | 延迟目标与持续命令率两项未过；不能因连接 1,000/1,000 宣称千人验收通过 |
| Gate 21 resource | success=false，CPU/内存不满足门要求 | 当前证据目录没有 gate20.json/gate21.json 最终通过报告，不能宣称 3,000 人目标完成 |
| Home Node Gate 25-local | accepted=true，但 scope=local-cryptographic-and-policy-only；productionHomeBetaAccepted=false | 真实三运营商网络等外部证据未提供；是项目附加平台能力，不能算 Crystal 游戏系统完成率 |

以上为读取现有产物及断言，没有在本轮重新运行集群或重新核验所有历史容器原始轨迹。[S28][S29][S30][S31][S32][S42]

安全抽查确认：新密码用 Argon2id；钱包账号隔离经典密码入口；Passkey 缺密钥默认拒绝；admin 路由统一认证并限流；生产拒绝 MoveTo、Stage5Command、裸 PasskeyLogin，未认证角色生命周期被拒绝。6 月安全审计开头的明文密码/读接口无鉴权已经有后续修复，不能照抄成现状。此轮没有执行渗透、依赖漏洞扫描或生产配置审计，也不宣称安全零缺陷。[S24][S25][S26][S27]

Android README 仍记录未具备 Android WebSocket transport 与 APK/设备证据，不能计作在线可玩客户端。钱包、家庭节点、资源分发属于项目附加能力，单列交付成熟度，不与 Crystal 玩法混算。[S34][S32]

**为什么此前观感与“通过率”不一致**

- 旧前端统计混合了不同日期和分母。本轮只读脚本复算到 280/284 个 inbound case 名、74/155 个字面 send 名、51 TSX/34 窗口命名文件；这些是词法覆盖，还含项目扩展协议。动态发包也可能不被字面统计捕获，因此两方向都不能直接转成玩法完整度。
- 旧资源报告可证明大量帧完成打包，却没有证明 Windows 运行时查得到并会绘制它们；当前实体单帧白名单就是具体断点。
- 个人模拟通过 AI、攻城、市场测试，并不保证共享在线路径会执行相同规则。
- 一个系统可能有 UI 图片、协议枚举、成功回包，却缺用户操作或真实的跨账号状态。例如交易格子和市场实例。
- 单次截图无法证明持续动作或时间序列正确；从小地图/新手怪物的局部验收不能推广全部地图与怪物。
- 现有语义 ledger 的 inventoryComplete=false 是合理的开放状态；源文件枚举器和测试计数不能替代对玩家行为的分类审核。[S01][S38][S39]

**建议按依赖顺序推进的验收队列**

| 顺序 | 交付范围 | 完成时必须拿出的证据 |
|---|---|---|
| 1 | 资产正确性与稳定基础旅程 | F01/F02 先补非破坏性隔离复现并修复；同一 Windows/Gateway 完成登录重进、连续移动、战斗/拾取/保存。现有掉落/交易守恒回归不倒退 |
| 2 | 全内容可见与可达 | 从 463 地图、刷怪、NPC、装备生成所需帧闭包；全库加载接入、缺图门、环境动画与多地图遍历；MineZones/事件/脚本遗漏建项 |
| 3 | 共享玩法完整化 | Boss/英雄、行会仓库/权限、全服市场、攻城逐个成为真实共享权威；双账号普通入口与断线/恢复/重复命令检验 |
| 4 | 两端操作与全职业内容 | 先补禁用/只读入口，再覆盖职业技能、NPC/任务、制作/精炼、社交/宠物等全部正反流程；每条链含保存重登 |
| 5 | 对照与发布 | 同版本语义轨迹、随机/时序边界、声画同状态对比、正式包及签名/资源授权、DPI/稳定性/目标容量与最终人工验收 |

这份队列是审计后的建议，没有在本轮自动执行修复、重打包、改账号、部署或把旧任务标记完成。全语义目录尚未闭合，因此也不提供虚假的总工期；应先把上述大域拆成可独立关闭的行为条目，再按实际产出估算。

**证据索引**

下列定位绑定本轮工作树；后续编辑会改变行号，请同时使用 source-snapshot.json 中的哈希。
- [S01] 全局语义完成标准：docs/CRYSTAL-SEMANTIC-PARITY-LEDGER.md:28
- [S02] 最新后端状态及掉落 Slice D：docs/BACKEND-1TO1-PROGRESS.md:756
- [S03] 普通市场/精炼/行会协议路径：apps/simulation/src/runtime/packets.rs:3872
- [S04] 个人 Stage5 系统与市场数据结构：apps/simulation/src/config.rs:6063
- [S05] 共享 Zone 怪物与战斗调度：apps/simulation/src/runtime/zone/runtime.rs:9360
- [S06] 共享模式个人 tick 明确排除 AI/英雄战斗：apps/simulation/src/runtime/monster_ai/mod.rs:638
- [S07] 个人沃玛教主专属 AI：apps/simulation/src/runtime/monster_ai/wooma_taurus.rs:15
- [S08] Crystal 沃玛教主：E:/mir2/Crystal/Server/MirObjects/Monsters/WoomaTaurus.cs:18
- [S09] NPC 解释器：apps/simulation/src/runtime/npc_script.rs:1133
- [S10] 地图事件可执行边界：packages/game-data/data/generated/crystal_map_event_manifest.json:726
- [S11] 原生实体资源选择：apps/game-client/platform-windows/src/atlas.rs:845
- [S12] 原生地图帧选择：apps/game-client/platform-windows/src/map_parser.rs:1211
- [S13] 原生地图打包默认范围：apps/web/scripts/build-native-keyed-map-pack.mjs:44
- [S14] 原生八项菜单禁用：apps/game-client/client-bevy/src/crystal_ui/overlays.rs:8477
- [S15] 交易格子呈现与交互边界：apps/game-client/client-bevy/src/crystal_ui/trade_dialog.rs:349
- [S16] 原生操作协议枚举：apps/game-client/platform-windows/src/native_protocol.rs:32
- [S17] Web 窗口实际挂载与 callbacks：apps/web/app/page.tsx:14220
- [S18] Windows 33 项用户反馈清单：docs/generated/player-qa/windows-visual-parity/VIS-03-USER-OBSERVED-UI-RENDER-BACKLOG-20260829.md:385
- [S19] 最新重复进入世界修复验收：docs/generated/player-qa/windows-reenter-world-20260908/README.md:13
- [S20] 最新持续奔跑修复验收：docs/generated/player-qa/windows-run-clock-20260908/README.md:16
- [S21] 最新人物相机修复验收：docs/generated/player-qa/windows-self-camera-20260908/README.md:11
- [S22] 最新自身攻击身份修复：docs/generated/player-qa/windows-owner-swing-20260908/README.md:1
- [S23] 持久化恢复日志：apps/gateway/src/save_recovery.rs:230
- [S24] 生产命令安全边界：apps/simulation/src/world_runtime.rs:124
- [S25] Argon2id 密码及钱包隔离：apps/simulation/src/runtime/save.rs:990
- [S26] 管理接口认证与限流：apps/admin-api/src/lib.rs:4359
- [S27] Passkey 缺密钥默认拒绝：apps/gateway/src/auth.rs:263
- [S28] 500 人 30 分钟历史验收：docs/generated/regional/gate18.json:1
- [S29] 500 人 1 小时与故障恢复历史验收：docs/generated/regional/gate19.json:1
- [S30] 1000 人负载未通过报告：docs/generated/regional/gate20-load.json:1
- [S31] 3000 人硬件门未通过：docs/generated/regional/gate21-resource-attestation.json:1
- [S32] Home Node 本地门与真实 Beta 边界：docs/generated/home-node/gate25-local/gate25-local-acceptance.json:1
- [S33] 发布及验收当前门：docs/AGENT-ORCHESTRATION.md:185
- [S34] Android 能力边界：apps/game-client/platform-android/README.md:49
- [S35] Crystal 原版市场实现：E:/mir2/Crystal/Server/MirObjects/PlayerObject.cs:8252
- [S36] Crystal 攻城状态机：E:/mir2/Crystal/Server/MirObjects/ConquestObject.cs:508
- [S37] 本地攻城状态：apps/simulation/src/runtime/stage5.rs:5000
- [S38] 旧前端统计不能代表当前完整度：docs/FRONTEND-COMPLETENESS-AUDIT.md:1
- [S39] 部分视觉分母，尚未完整：docs/generated/player-qa/windows-visual-parity/phase-a-denominator.json:1
- [S40] 最新交易金币保管证据：docs/generated/player-qa/native-ui-parity-20260908-trade-gold/README.md:1
- [S41] Crystal NPC 脚本语法：E:/mir2/Crystal/Server/MirObjects/NPC/NPCSegment.cs:102
- [S42] 正式容量标准：docs/REGIONAL-ACCEPTANCE.md:1
- [S43] 当前实体图集清单：apps/web/public/bevy-entity-atlases/manifest.json:1
- [S44] 矿区导入丢弃位置：packages/tooling/scripts/generate-crystal-respawn-manifest.mjs:397
- [S45] 原生动作回退规则：apps/game-client/platform-windows/src/frame_sets.rs:51
- [S46] 完整内容配置含义：apps/gateway/src/main.rs:31
- [S47] 安装包 keyed 地图清单：C:/mir2-ground-label-20260908/mir2-assets/generated/native-map-keyed/manifest.json:1
- [S48] 安装包普通地图图集：C:/mir2-ground-label-20260908/mir2-assets/generated/map-atlas/manifest.json:1
- [S49] Windows 打包资源范围：apps/game-client/platform-windows/scripts/package-windows-candidate.ps1:768

[S01]: <E:/mir2-player-journey/mir2-web3/docs/CRYSTAL-SEMANTIC-PARITY-LEDGER.md:28>
[S02]: <E:/mir2-player-journey/mir2-web3/docs/BACKEND-1TO1-PROGRESS.md:756>
[S03]: <E:/mir2-player-journey/mir2-web3/apps/simulation/src/runtime/packets.rs:3872>
[S04]: <E:/mir2-player-journey/mir2-web3/apps/simulation/src/config.rs:6063>
[S05]: <E:/mir2-player-journey/mir2-web3/apps/simulation/src/runtime/zone/runtime.rs:9360>
[S06]: <E:/mir2-player-journey/mir2-web3/apps/simulation/src/runtime/monster_ai/mod.rs:638>
[S07]: <E:/mir2-player-journey/mir2-web3/apps/simulation/src/runtime/monster_ai/wooma_taurus.rs:15>
[S08]: <E:/mir2/Crystal/Server/MirObjects/Monsters/WoomaTaurus.cs:18>
[S09]: <E:/mir2-player-journey/mir2-web3/apps/simulation/src/runtime/npc_script.rs:1133>
[S10]: <E:/mir2-player-journey/mir2-web3/packages/game-data/data/generated/crystal_map_event_manifest.json:726>
[S11]: <E:/mir2-player-journey/mir2-web3/apps/game-client/platform-windows/src/atlas.rs:845>
[S12]: <E:/mir2-player-journey/mir2-web3/apps/game-client/platform-windows/src/map_parser.rs:1211>
[S13]: <E:/mir2-player-journey/mir2-web3/apps/web/scripts/build-native-keyed-map-pack.mjs:44>
[S14]: <E:/mir2-player-journey/mir2-web3/apps/game-client/client-bevy/src/crystal_ui/overlays.rs:8477>
[S15]: <E:/mir2-player-journey/mir2-web3/apps/game-client/client-bevy/src/crystal_ui/trade_dialog.rs:349>
[S16]: <E:/mir2-player-journey/mir2-web3/apps/game-client/platform-windows/src/native_protocol.rs:32>
[S17]: <E:/mir2-player-journey/mir2-web3/apps/web/app/page.tsx:14220>
[S18]: <E:/mir2-player-journey/mir2-web3/docs/generated/player-qa/windows-visual-parity/VIS-03-USER-OBSERVED-UI-RENDER-BACKLOG-20260829.md:385>
[S19]: <E:/mir2-player-journey/mir2-web3/docs/generated/player-qa/windows-reenter-world-20260908/README.md:13>
[S20]: <E:/mir2-player-journey/mir2-web3/docs/generated/player-qa/windows-run-clock-20260908/README.md:16>
[S21]: <E:/mir2-player-journey/mir2-web3/docs/generated/player-qa/windows-self-camera-20260908/README.md:11>
[S22]: <E:/mir2-player-journey/mir2-web3/docs/generated/player-qa/windows-owner-swing-20260908/README.md:1>
[S23]: <E:/mir2-player-journey/mir2-web3/apps/gateway/src/save_recovery.rs:230>
[S24]: <E:/mir2-player-journey/mir2-web3/apps/simulation/src/world_runtime.rs:124>
[S25]: <E:/mir2-player-journey/mir2-web3/apps/simulation/src/runtime/save.rs:990>
[S26]: <E:/mir2-player-journey/mir2-web3/apps/admin-api/src/lib.rs:4359>
[S27]: <E:/mir2-player-journey/mir2-web3/apps/gateway/src/auth.rs:263>
[S28]: <E:/mir2-player-journey/mir2-web3/docs/generated/regional/gate18.json:1>
[S29]: <E:/mir2-player-journey/mir2-web3/docs/generated/regional/gate19.json:1>
[S30]: <E:/mir2-player-journey/mir2-web3/docs/generated/regional/gate20-load.json:1>
[S31]: <E:/mir2-player-journey/mir2-web3/docs/generated/regional/gate21-resource-attestation.json:1>
[S32]: <E:/mir2-player-journey/mir2-web3/docs/generated/home-node/gate25-local/gate25-local-acceptance.json:1>
[S33]: <E:/mir2-player-journey/mir2-web3/docs/AGENT-ORCHESTRATION.md:185>
[S34]: <E:/mir2-player-journey/mir2-web3/apps/game-client/platform-android/README.md:49>
[S35]: <E:/mir2/Crystal/Server/MirObjects/PlayerObject.cs:8252>
[S36]: <E:/mir2/Crystal/Server/MirObjects/ConquestObject.cs:508>
[S37]: <E:/mir2-player-journey/mir2-web3/apps/simulation/src/runtime/stage5.rs:5000>
[S38]: <E:/mir2-player-journey/mir2-web3/docs/FRONTEND-COMPLETENESS-AUDIT.md:1>
[S39]: <E:/mir2-player-journey/mir2-web3/docs/generated/player-qa/windows-visual-parity/phase-a-denominator.json:1>
[S40]: <E:/mir2-player-journey/mir2-web3/docs/generated/player-qa/native-ui-parity-20260908-trade-gold/README.md:1>
[S41]: <E:/mir2/Crystal/Server/MirObjects/NPC/NPCSegment.cs:102>
[S42]: <E:/mir2-player-journey/mir2-web3/docs/REGIONAL-ACCEPTANCE.md:1>
[S43]: <E:/mir2-player-journey/mir2-web3/apps/web/public/bevy-entity-atlases/manifest.json:1>
[S44]: <E:/mir2-player-journey/mir2-web3/packages/tooling/scripts/generate-crystal-respawn-manifest.mjs:397>
[S45]: <E:/mir2-player-journey/mir2-web3/apps/game-client/platform-windows/src/frame_sets.rs:51>
[S46]: <E:/mir2-player-journey/mir2-web3/apps/gateway/src/main.rs:31>
[S47]: <C:/mir2-ground-label-20260908/mir2-assets/generated/native-map-keyed/manifest.json:1>
[S48]: <C:/mir2-ground-label-20260908/mir2-assets/generated/map-atlas/manifest.json:1>
[S49]: <E:/mir2-player-journey/mir2-web3/apps/game-client/platform-windows/scripts/package-windows-candidate.ps1:768>


## 审计后的修复记录（不改写原始快照）

2026-09-08：F01/F02 的市场及精炼材料完整实例保管子问题已完成代码修复与隔离测试，新增 14 个回归用例通过；详见 [修复证据](generated/player-qa/market-refine-custody-20260908/README.md)。跨账号共享市场、精炼武器炉内保管和计时仍未完成，因此不将这两组完整玩法验收标记为关闭，也不计算全游戏完成百分比。
