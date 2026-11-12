# Windows R16 与 Web 1:1 验收

2026-11-12 正式站c6bc815bae的公网签名/进城/移动/完整画面/共享HUD/繁中Q/退出通过，原108/113预取失败保留。HUD补读候选通过177项和构建：最多8个实际失败URL、并发2、一次补读，配置ACK有界、保留初始与取消日志，完整正文后才更新计数；HUD后并行字库/运行包。期间Gateway历史前进但未改变Web/客户端、未部署Gateway；65文件保全。待新源码公开复验113/113，120秒冷启和整体1:1开放。[补读候选](evidence/r16-hud-recovery-20261112/README.md) · [当前公网分项](evidence/r16-runtime-delivery-20261112/production/README.md)

2026-11-12 本地字库并行/同版镜像恢复通过132项、生产构建及两后端重开/保存。受控HUD38.1→26.3秒；共享WebGL2六次首入口头超时后完整备用字库、113/113、Q和移动通过。实际CDN+故障应用源仍超过原120秒，后续恢复及约0.8秒缓存重开分别保留；空闲前缀正常，继续诊断启动排队。候选未发布、正式站仍72c00d20ae，原65个文件保全，完整1:1、三职业当前路线、Hosted与实体手机等仍开放。[本地候选](evidence/r16-font-overlap-20261112/README.md) · [已发布版本](evidence/r16-native-release-20261112/README.md)

2026-11-12 已按用户部署要求发布 `51d2dcc346`。公网简繁出码、手机尺寸官方Testnet钱包签名、进比奇、权威移动、共用HUD/任务窗、113/113预取及正常退出通过，无需回滚。新云端图集摘要/缓存身份与计算相同；Worker正常更新一次，未迁移旧缓存。本机75项、生产构建及两后端重开同档通过。继续冷加载长尾、实体手机和既有未结项；100% Candidate、完整三职业/地图/跨端及编译来源仍未完成。[正式回执与历史失败](evidence/r16-cache-repair-20261112/README.md)

2026-11-11 正式版 ef0d1ee9dc 优先读取比奇地砖和玩法词库，按所选角色读取人物素材，NPC 与建筑分包补齐。隔离浏览器通过 12 秒慢响应头及晚到建筑，同一旧档可移动和退出存档。公网繁中扫码签名、比奇首帧、实际移动和退出通过。继续公网旧档、首步长尾、其他渲染路径和实体手机。 [同档、公网与回滚回执](evidence/r16-visual-packs-20261111/README.md)

2026-11-11 已将任务文字候选 78c900ca36 正式发布到 numeron.obelisk.build/game。固定源码与 11 个启动包经发布前后校验，普通 Chrome 简繁出码、Testnet 签名进城、真实一步、繁中备用任务窗和正常退出通过。共享 GPU 任务窗、实体手机、公开旧角色冷重登及毕业目标选择仍开放；整体 1:1 Goal 继续。 [发布与公网回执](evidence/r16-quest-release-20261111/README.md) · [前置任务接交与重启](evidence/r16-quest-text-20261111/README.md)

2026-11-11 正式版 `5c4df0c1c2` 修复比奇地面包提前中止与快选时未画地面就开放输入。相同旧档验证持续正文、停传重试和普通散图。公网繁中签名进城、首帧 221 张原图地砖、实际走一格与退出通过。首步往返 2.063 秒，网关受理计数对应 650 微秒。[同档与公网回执](evidence/r16-floor-progress-20261111/README.md)

2026-11-11 正式版 `f512cfbc99` 将十语玩法词库改为固定地址校验与可重试读取。简繁冷读异常恢复、比奇文字和繁中备用任务窗的原版标题说明通过。繁中公网签名进比奇并正常退出，词库在进城前就绪。地面包仍在有正文进展时于 12 秒取消，移动权威回包超过 8 秒验收窗。继续这两条长尾、备用任务窗剩余文字和实体手机。 [词库和公网回执](evidence/r16-gameplay-locale-20261111/README.md)

2026-11-11 正式版 `d740f13823` 公网扫码和官方 Testnet 签名到比奇、走路、正常退出通过。首个可操作画面后约 4 秒采样见 9 个原版怪物身体。隔离原版行走帧通过回包重放，已有角色从上次落点进场，在怪物包被扣住时可移动。继续公网已有角色、自然行走和战斗帧、语言正文延迟、其他渲染路径与实体手机。 [正式发布与分项回执](evidence/r16-entry-monsters-20261111/README.md) · [小地图跨区](evidence/web-r16-parity-goal-20261107/minimap-long-route-live.json)

2026-11-11 公网已发布 `e3e6f3f06f`。隔离生产构建一次读完持续 14.35 秒的地图正文，实际移动和正常注销读档通过。公网简繁出码、Testnet 签名、比奇权威回包和退出通过。该次默认网络两次地图请求未收到源站响应头，继续复验可操作首帧与首次移动。 [正文对照和正式发布](evidence/r16-map-body-progress-20261111/README.md)

2026-11-11 公网已发布 `534322e790`，含 `6997f810ed` 的玩家反馈修复。首帧玩法词库解码体积减少 27.92%，角色元数据在地图解码后请求。隔离 Chrome 验证移动、NPC 词库失败及重开后的真实商店回包。公网简繁出码、Testnet 签名、进入比奇的权威回包及退出通过。默认网络会话中核心词库已就绪，地图正文仍超时，可操作首帧和首次移动待验。 [本轮发布和复验](evidence/r16-npc-cold-entry-20261111/README.md) · [上轮地图分段回放](evidence/r16-pinned-scene-entry-20261110/README.md)

2026-11-10 玩家反馈优先。隔离生产构建的宽屏退出、血蓝及实际双人占位后寻路通过。十语扫码到待确认、登录、选角和快速进比奇的已验范围分别记录。生产切换、攻击动作时序和现场入口继续验收。[本轮复测](evidence/r16-player-feedback-20261110/README.md)

2026-11-10 用户在公网扫码修复后重新激活持续 Goal，沿 `codex/windows-player-journey` 继续全部未结范围，保留现有工作和历史失败。目标为适用代码、数据、测试和 QA 门槛的 100% Candidate；最终真人 Accepted 仍需本人游戏与界面验收。优先无需控制 Windows 桌面的代码、普通协议、无界面浏览器和构建。本机三职业到 30 级与公开服三职业首任务、中文 Q 和扫码进城验证分开记录。[公网扫码](NUMERON-WEB-RELEASE-20261108.zh-CN.md#2026-11-10-公网钱包扫码恢复)

2026-11-07 建立持续 Goal。基准为 `codex/windows-player-journey` 在 `mir2-player-journey` 工作树的 Windows R16。复核时本地基点为 `c404bde8a`，远端为 `a7dc19d385b8`。远端额外七个提交只涉及官网和 Atlas。当前工作树中游戏、Windows、Web 的未提交内容继续保留，代码和已运行包分别标记。[分支比较](https://github.com/Zombieliu/mir2/compare/c404bde8a5190534f6bc83ec8a4fe78fffc8cb50...a7dc19d385b85a8205f1f29dea25a54a1dbd85ba)

`r16-gameplay-baseline.json` 定义两端共用的五职业、`newcomer-v2` 和 Native 引导。对齐按同一内容和 Gateway 下可观察的界面、动作、结果、失败反馈与重登状态逐项验收。每项记录源代码位置、候选构建和实际证据。键鼠、浏览器系统能力等输入和宿主差异单列。项目的 100% Candidate、真人 Accepted 和正式发布分别核对。[共享配置](../packages/game-data/data/r16-gameplay-baseline.json) · [门槛](AGENT-ORCHESTRATION.md)

| 范围 | 已核对 | 未完成及验收要求 |
| --- | --- | --- |
| 登录、选角、创角 | Web 过场、重名重试、断线重登和四槽权威已验。两端创角标签已对齐布局。Web 注销按 Native 返回已认证选角。快捷键改用稳定订阅，两后端各四进四出且导航期间不重绑。公网已验简繁出码、签名、创建角色、选角、首屏地面、建筑、主角与 NPC、走路和退出。 [回执](evidence/r16-entry-visuals-20261111/README.md) | 比奇怪物原版站立帧已有[公网回执](evidence/r16-entry-monsters-20261111/README.md)。继续公网已有角色快进、Windows 窗口、旧超额档和实体手机验收 |
| 布局、语言、设置 | Native 十语和字体共用。毕业装备与 D605 名称、十语窄窗和 65 次关闭零移动有证据。首次 Q 的渲染间隙覆盖已修，中、英、阿八次独立冷启动和角色默认页签复测通过。游戏帮助的十语标题、快捷键、九语 42 页正文及阅读位置已对齐 | 旧 headless-shell 的约 61 秒主线程停顿待兼容调查。继续核对任务详情、交付地、奖励、追踪、设置和键位。既有 Native 译文仍需校对 |
| 移动、地图、遮挡 | 两端共用碰撞原数据与行深度规则。Web 大地图有普通位移回执。小地图左右键、缩放、标题停止和 Q 日记拦截已复测。比奇摊位与 NPC 的混合画布遮挡已修，三条渲染路径验证同排显示及前排建筑遮挡。比奇战斗点稀疏地格冷重登已用两条 Web 渲染路径实测打怪和存档。 [重登入场](evidence/r16-combat-input-20261110/sparse-scene-resume.json) | 继续核对小地图长路线、占位、移动中的区域切换、旧地图和重连输入。Windows 实机与其他地图的场景叠放另验 |
| 战斗、技能、药与拾取 | Web 实测天然击杀、拾取、Digit1 用药及三职业技能说明。三职业 30 级档绑键与读档通过，战士半月开关和一秒间隔、法师火墙、道士召唤和治疗有实测回包。大火球与灵魂火符按 Crystal 延时命中，七格、熟练度重登及单次扣符通过。方向飞行、移动目标命中、地形上方绘制和三段原声有本机 Web 证据。两条生产浏览器路径已复测走跑接灵魂火符、施法后首步、回原格再施法和读档。桌面腰带与背包共享 300 毫秒间隔，普通 NPC 对话开关、普通背包内外点击和野怪命中有生产浏览器对照。逐格追击、死亡停手及右键覆盖怪物和 NPC 走位通过实测。两路复测经典右键松手、动作间隔短按及追上移动奥玛，关设置后同一会话继续新版寻路。透明树像素命中后方奥玛，实色处选中栗树。 [用药与背包](evidence/r16-combat-input-20261110/README.md) · [追击与右键](evidence/r16-ordinary-chase-20261110/README.md) · [释放、设置与移动怪物](evidence/r16-right-release-20261110/README.md) · [逐像素命中与冷却短按](evidence/r16-actor-picking-20261110/README.md) | 继续纯 GPU 图集选怪、其余技能动作与伤害时序、防御取样、武器准备技和掉落归属。核对其他窗口重叠、Windows 实机与正式发行 |
| 钓鱼 | R16 鱼具窗、真实渔获和读档已验。WebGPU 与 WebGL2 实点验证原图跟手、双向装卸、归零提示和线轮移除后关闭续抛。普通 Web 和二进制 TCP 验证零耐久空竿续抛和同档替换。Windows 屏外已验原始动作、水纹、声音和按钮，冷旁观者即时收到状态边，重进视野恢复等待 | 继续 Windows 窗口、Hosted 和正式发行。连续续抛时的早咬钩短探针超时已保留 |
| 任务、地图和同档 | 三职业各 22 主线及 4 成长至 30 级，Web 重登读回。三份毕业档在隔离服通过 Web → Windows 正式传输 → 新 Web 会话往返，九次正常注销，任务和个人状态落盘一致 | 道士原 120 分钟到 27 级，同档另续至 30。需复验矿洞 14–16 项限时、Windows 窗口渲染及后续区域的独立范围 |
| 组队与交易 | Web 复用 Native 悬停目标与八项社交预算。普通组队、待回包取消、改价和换物后重确认、双向交割和同角读档通过。独立报价支持满包解锁、退件及 File 意外重启，真实 PostgreSQL 验证成交与退回的来源围栏。Native 报价识别旧编号 `0`，公会本机生产冷双开通过。本方确认后两次同额加价、双方重确认、交割和重登通过。交易邀请复用系统提示和两秒冷却。生产 Web 双开通过混合大小写邀请、移除后同连接重邀、拒绝和静默冷却后的手动重邀。Windows 命令队列验证同名重邀 | 继续公会招募与建职阶、Windows 窗口及完整 Hosted／TCP 链路。发行需更新预编译 Web 运行包 |
| 阿洛 | 普通入队、自然语言教学、签名档案与新 SQLite 导入已有证据。实际 Web v2 战后拾取、采购扣款和返程、生活授权恢复通过。Web 教学、重启、Windows 正式传输读回及撤销、新 Web 会话读回同一 v3 档案通过。Web 面板随十种游戏语言切换，简中、英文、阿语复测同一教学与授权 | 六种语言的 12 条英文回退、其余语言的实际流程、Web 模型设置、Windows 窗口和跨端实战继续验收，另验从实际 UI 授权后的完整离线生活、扣款与返家 |
| Crystal 原英雄 | 两端已有英雄窗口及部分普通协议适配 | 当前两端都缺完整的普通创建与召退闭环，槽位切换也欠权威存档证据。列为共用缺口，分别验普通权限和跨端读回 |
| 资源与发行 | v21-r3 的本机有限资源 42,724 项，v22 共享服已做三职业功能验收。有限包完整索引误宣告能力已修，默认 WebGPU 进出和中／阿首次 Q 均无资源错误 | 同一发行构建验证冷缓存、CDN 路径和 360 MiB 门槛。对公网默认网络分别测资源清单、地图正文、解码与可操作首帧。首屏资源按实际落点准备，后台补齐后续区块。正式 Gateway 为 `platinum_176`，Linux 签发、正式库旧档恢复和 R16 配置迁移另验 |

来源与已有实测见 [本机试玩](WEB-R16-PLAYABLE-HANDOFF-20261104.zh-CN.md)、[十语与早期同档](WEB-R16-FOREST-SHARED-SAVE-20261106.zh-CN.md)、[三职业毕业档往返](evidence/r16-shared-save-roundtrip-20261110/README.md)、[三职业 v22](WEB-R16-NEWCOMER-V22-20261107.zh-CN.md)、[公会双开](evidence/r16-guild-cold-entry-20261110/README.md)、[交易解锁](evidence/r16-trade-reopen-20261110/README.md)、[阿洛自主生活](ALO-AUTONOMOUS-LIFE-20261103.zh-CN.md)、[原生验收](NATIVE-WINDOWS-PLAYER-QA.md)。

本轮复测记录包括[对端确认后的报价修改](evidence/r16-trade-bilateral-edit-20261110/README.md)、[三职业技能快捷键](evidence/r16-skill-shortcuts-20261110/README.md)、[阿洛跨端培养](evidence/r16-alo-cross-client-20261110/README.md)、[本方确认后的加价及交易邀请](evidence/r16-trade-own-gold-20261110/README.md)、[组队名字回执](evidence/r16-group-canonical-name-20261110/README.md)和[组队手动重试](evidence/r16-group-invite-retry-20261110/README.md)。

阿洛面板的简中、英文和阿语浏览器实测包含登录、固定教学或读回、查看补给授权和正常退出。记录十种语言的固定按钮和未译文案范围。 [面板语言验收](evidence/r16-alo-localization-20261110/README.md)

比奇商人遮挡已用同一位置和资源对照旧版与修复版。混合回退时由 DOM 同时排序人物和遮挡物，固定图集验证全世界 GPU 接管。 [像素、移动和图集证据](evidence/r16-bichon-npc-depth-20261110/README.md)

七格施放大火球和灵魂火符已用普通野怪复测击杀、资源和熟练度读回。共享服务器设置 850 毫秒结算，Web 另验两项技能的位图、跟随目标和三段音效。浏览器与服务端分别取样，记录了伤害包和画面时钟的差异。 [服务器计时](evidence/r16-delayed-projectiles-20261110/README.md) · [客户端阶段](evidence/r16-web-spell-phases-20261110/README.md)

旧包连续走跑会积压身体动作。复用 Windows 队列规则并同步轻量呈现 WASM 后，WebGL2-shared 和默认 WebGPU 以普通输入通过走跑接灵魂火符。双方均记录服务器伤害回执，WebGPU 实拍可见浮字。[对照与构建](evidence/r16-web-move-cast-20261110/README.md)

施法后首步的旧标识重播已在两条本机生产浏览器路径复验。独立 WebGPU 在普通野怪施法后走到岸边，使用共享画布世界指针抛竿，收到真实服务器回执并读回扣饵和鱼竿耐久。后续两条浏览器路径复验了冷登录后的站姿恢复和收竿。Web 普通鱼具装卸同角重登、自动重抛关闭和真实渔获有分项证据。 [施法与抛竿](evidence/r16-web-postcast-20261110/README.md) · [钓鱼窗口与回执](evidence/r16-web-fishing-loop-20261110/README.md)

首竿时序沿用同一比奇岸边存档。Web 在默认 WebGPU 用普通点击，Windows 走无界面正式传输。有效抛竿唤醒共用 Gateway 后，两端在 0.70 秒内收到第一个进度包，四轮注销均读回单次抛竿消耗。 [时序与编译记录](evidence/r16-web-fishing-loop-20261110/cold-first-cast-timing.json)

比奇进城在本机通过真实网络延迟下的限时重试、玩家按钮恢复和注销时过期请求隔离。钓鱼已在两条 Web 渲染路径验证原版水面与声音，在 Windows 优化构建验证原始素材坐标与图层。 [进城恢复](evidence/numeron-web-release-20261108/first-scene-recovery-local-20261110.json) · [钓鱼画面和回执](evidence/r16-web-fishing-loop-20261110/fishing-cues.json)

满包独立交易格已通过浏览器解锁、原格取回、单次交割和双方重登。File 强停复测区分未成交退回与已成交补账，真实 PostgreSQL 验证迟到确认不能结算已退回报价。 [正常、强停和真实库证据](evidence/r16-trade-restart-20261110/README.md)

## 小地图标题和边框

11 月 11 日，兼容渲染路径用隔离 File 战士实点 MMap101 一次，从 `(328,264)` 走到 `(328,218)`。2 次走路和 22 次跑步展开为 46 格，穿过原区域 `y=223` 时镜头和权威坐标均在 `(328,222)`。同角正常退出和重登均读回目标格。离线第三格占位差异仍待真实双角色验收。 [路线回执](evidence/web-r16-parity-goal-20261107/minimap-long-route-live.json) · [终点画面](evidence/web-r16-parity-goal-20261107/minimap-long-route-destination.png)

Windows 的小地图框架消费左右键，先结束已有路线。Web v21-r3 用相同比奇存档启动 13 格路线，点击标题后仍发出五次 Run，并到达原目标。Web 在小地图框架接入既有路线取消，图像点击和地图按钮各由原入口处理。回归直接执行实际 JSX 事件，覆盖左右键、图像子节点、按钮和中键。

在隔离的 v22 File 存档副本，通过新源码的本机 Next 页面，1280×960、1024×768、960×720 的六次普通图像点击均有 Walk/Run 和指定格的权威坐标。窄窗口实际将小地图视图缩为 112.5×101.25。左右键分别点标题，后续移动指令均为零，路线停在目标之前。Q 日记打开时点击为零移动。结束有正常注销和干净的浏览器网络检查。首次窄窗脚本用了分数坐标，事件量化落到邻格，后续用实际可点的整数坐标复测。Web 相关 24 项回归通过。[前后对照收据](evidence/web-r16-parity-goal-20261107/minimap-web.json) · [窄窗截图](evidence/web-r16-parity-goal-20261107/minimap-web.png)

已验证比奇近距离短路线、标题和 Q 日记场景。历史远距离点击未起步的记录继续单列，其他地图切换、占位和旧图输入待核对。修复以源码验证记录，当前试玩入口仍是此前的包。

## Web 创角回执

Windows R16 等待服务端回执后才切回选角，失败保留窗口。Web 按当前 Gateway 会话提供允许的职业，保留单笔提交直到收到对应回执。成功以服务端索引插入并选中新角色，支持 R16 预览回执的 `level=0`。失败保留名字、职业和性别。回执超过 15 秒或提交后断线时，提示重新登录检查角色列表，随后由玩家决定是否重试。

空白 File 服使用 v22 Gateway 和当前 Web 源码，通过普通账号实测。五职业服在写档被拒后保留草稿，编辑后延迟回执期间双击和 Enter 无重复发送；选中新角色、进城和正常注销通过。隐藏表单时仍保留请求，迟到成功回执能选中第四个角色。分别丢弃成功回执和在成功后关闭连接，浏览器重登均只看到一条对应新角色；旧三职业服提供三种选择且实际创角通过。53 项选定回归和 TypeScript 检查通过。截图使用独立复验，在登录过场退场后截取，角色名字已遮罩。[浏览器收据](evidence/web-r16-parity-goal-20261107/character-create-web.json) · [失败保留草稿](evidence/web-r16-parity-goal-20261107/character-create-rejected.png) · [等待回执](evidence/web-r16-parity-goal-20261107/character-create-pending.png)

原 v22 的负向实测确认跨账号重名及第五槽均能在 File 落盘。过场期间可点击下层选角，五职业性别标签被遮挡。各项修复与复测见下文。

## 共享创角校验

正常创角先检查四槽，再检查全服在册角色名，ASCII 字母忽略大小写。File 在现有持久化事务中检查，PG 在事务中锁住创角与账号写入边界，防止两个网关同时占名、占最后一槽或使用同一新索引。只重试提交前确认的索引冲突。普通删除沿用 R16 的释放名字行为，Crystal 原版软删保留名字另列。旧重名和超额档保留，无数据库迁移。隔离旧档中第五个角色经正常协议可进游戏，界面仍按四槽显示。

File 和原生 PostgreSQL 17.11 的并发、重开、旧档等级与金币、释放名字各有验证。两种回执共用 Native 词库。Web 收到 `5` 保留草稿并聚焦名字，收到 `4` 关窗并在选角显示满槽。实际浏览器中另一个普通会话补满剩余槽，旧页面创角收到 `4`，重登精确读回四个原索引，改名角色可进城及正常注销。首次复验的拒绝流程通过，进城回执超出 12 秒观察窗口。相同 Gateway 二次用 90 秒窗口完整通过。Windows 回执、状态和防重共四项测试通过，平台锁文件的依赖差异在外置测试副本离线解析，正式锁定构建与窗口取证待验。Web 54 项选定回归和 TypeScript 通过。[测试和源文件摘要](evidence/web-r16-parity-goal-20261107/character-constraints-verification.json) · [首次收据](evidence/web-r16-parity-goal-20261107/character-constraints-browser-first.json) · [完整收据](evidence/web-r16-parity-goal-20261107/character-constraints-browser.json) · [重名截图](evidence/web-r16-parity-goal-20261107/character-constraints-name.png) · [满槽截图](evidence/web-r16-parity-goal-20261107/character-constraints-slots.png)

## 登录开门与创角标签

修复前用普通账号实测，门帧播放中点击旧创建坐标就打开了下层创角窗。五职业的职业和性别标签均与按钮相交。当前 Web 收到有效 `LoginSuccess` 后进入独立开门状态。19 张原帧准备时停在第 0 帧，随后按 Windows 的 100 毫秒间隔播放 1–18，结束后才为当前 socket 和 generation 开放选角。断线、换连接和回登录使旧完成信号失效。浏览器单帧加载失败或等待达到 30 秒时使用已认证角色列表进入选角，缺帧路径独立记录。

相同隔离 Gateway 的浏览器复测扣住第 8 帧，Tab、Enter、空格、Esc 和四个旧按钮坐标都未发选角命令。重送一份原登录回执未提前开放，压住鼠标跨过退场时释放也未触发创角。从第 1 帧到选角测得 1818 毫秒。关连接后迟到资源未揭开旧列表，重新登录由新连接读回原角色。故意拒绝一张门帧时，已认证列表仍可用。另一次实测只转发真实 `StartGame(4)` 并暂扣后续启动包，点击退出正常回登录。前一次仅扣两种包时进城先发生，保留为测试边界记录。[退出回执](evidence/web-r16-parity-goal-20261107/login-door-exit.json) · [最终源码复测](evidence/web-r16-parity-goal-20261107/login-door-browser-final-source.json)

两端将本地化标签移到各按钮行之前，提示移到 Windows 原提示坐标。Web 1280×960 简中五职业及 960×720 葡语三职业均验标签完整与实际创角，五职业新角正常进城和注销。第一次复验因脚本把葡语两个标签的 DOM 顺序写反而停止，修正断言后完整通过。Web 67 项选定回归及 TypeScript 通过，Native 6 项开门、音频及创角测试通过。Native 窗口及发行包另验。[缺陷复现](evidence/web-r16-parity-goal-20261107/login-door-before.json) · [首次复测](evidence/web-r16-parity-goal-20261107/login-door-browser-first.json) · [完整复测](evidence/web-r16-parity-goal-20261107/login-door-browser.json) · [源文件摘要](evidence/web-r16-parity-goal-20261107/login-door-verification.json) · [开门](evidence/web-r16-parity-goal-20261107/login-door-cold.png) · [修复前布局](evidence/web-r16-parity-goal-20261107/create-labels-before.png) · [五职业](evidence/web-r16-parity-goal-20261107/create-labels-five-zh-CN.png) · [窄屏三职业](evidence/web-r16-parity-goal-20261107/create-labels-classic-pt-BR.png)

## 毕业建议、十语与关闭手势

毕业日记按装备索引和导入地图索引读取共用 Native 名称。战士使用 `244` 的“裁决之杖”，法师使用 `259` 的“战法法杖”，道士使用 `272` 的“灵泉魔杖”。D605 对应导入地图 `186`，简中显示“地牢二层北部”，挑战标题继续使用毕业配置中的“清剿昆虫洞穴北二层”。展示与原有获取说明一同进入共用日记。Web 有限词库补入这四个既有词条，生成结果为 2,237 条、1,187,023 字节，通过现有 1.25 MiB 预算。[源码、构建及结果收据](evidence/web-r16-parity-goal-20261107/graduation-final-verification.json)

共用 Rust 测试覆盖十语、三职业，以及桌面和移动日记的 60 组标题。7 项 Native 测试、104 项 Node 回归和 TypeScript 检查通过。完整重建 WebGPU、WebGL2 和 WebGL2-shared 包，版本为 `bevy-db5e53a91f4d11de`，六个文件和不可变副本的 SHA256 相符，三套体积检查通过。浏览器实测使用 WebGL2-shared 的共享 UI。[Native 测试](evidence/web-r16-parity-goal-20261107/graduation-final-native.log) · [有限词库测试](evidence/web-r16-parity-goal-20261107/graduation-final-node.log)

修复前，阿语一次复测关闭日记后发出九次 Walk，战士副本从 `(328,264)` 走到 `(328,255)`。此前脚本只约束资产与任务，仍将该次标为通过。首条位移发生在 Q 阶段，终点与关闭按钮下方的地图格吻合。Shell 现在利用已有 Quest 指针记录保留整个关闭手势的归属，覆盖窗口在按下时消失、前景矩形尚未同步的情况，并保留未遮挡 HUD 的交互。对应真实处理函数的负向回归先失败，修复后覆盖 up、cancel、两个画布和后续世界点击。[误移动原收据](evidence/web-r16-parity-goal-20261107/graduation-final-ar-unintended-walk.json) · [修复前回归](evidence/web-r16-parity-goal-20261107/graduation-final-pointer-before.log) · [修复后回归](evidence/web-r16-parity-goal-20261107/graduation-final-pointer-after.log)

最终以三份既有 30 级毕业档独立登录，均实读 22 项主线与 4 项成长，打开 Q、选择装备和挑战、查看 F11 技能并正常注销。另以 `960×720` 浏览器窗口逐一通过十语。每次分别用 80／200 毫秒按住日记 X，再从 HUD 关闭，共 13 次独立登录、65 次关闭手势，整场 walk、run、turn 及战斗、用药、任务操作均为零，位置保持一致。[战士](evidence/web-r16-parity-goal-20261107/graduation-final-warrior-zh-CN.png) · [法师](evidence/web-r16-parity-goal-20261107/graduation-final-wizard-zh-CN.png) · [道士](evidence/web-r16-parity-goal-20261107/graduation-final-taoist-zh-CN.png) · [俄语窄窗](evidence/web-r16-parity-goal-20261107/graduation-final-narrow-ru.png) · [阿语窄窗](evidence/web-r16-parity-goal-20261107/graduation-final-narrow-ar.png)

原三台 v22 Gateway 持续在线并更新世界时钟。本轮 Gateway 使用独立复制的 File 数据，逐职业比较等级、经验、金币、背包、装备、任务、技能等 26 项个人字段，78 项相等。事件、revision 与坐标差异分别保留，战士副本保留修复前九步移动的结果，最终复验从该位置开始。最早一次阿语冷启动按 Q 无回执，后续独立复测及最终加强复测通过，首次输入问题继续排查。现有 Native 译文的夹杂英文继续列入文案校对。[字段比较与范围](evidence/web-r16-parity-goal-20261107/graduation-final-verification.json) · [首次 Q 失败](evidence/web-r16-parity-goal-20261107/graduation-final-ar-first.json)

本轮通过隔离的 Next 源码环境验证新渲染包。验后关闭本轮 `19348` 与 `19244–19246` 服务，保留构建、测试副本和证据。Windows 窗口、正式发行构建、360 MiB 资源门槛和真人验收继续按各自范围推进。[清理收据](evidence/web-r16-parity-goal-20261107/graduation-final-cleanup.json)

## 注销选角、首次 Q 与资源能力

Windows R16 的 `LoggedOut` 保留账号和连接，按稳定角色 ID 在服务端列表中恢复选择；`SessionReset` 恢复角色第一页和背包第一页。Web 现按相同规则处理当前会话已申请的注销回执：先撤销旧角色的窗口、请求和私有状态，再用权威列表返回选角，保留认证。点击选角的离开按钮才关闭连接和清除认证。旧连接、旧 generation、未申请、重复及无效列表不能获得新选角权限。[源文件、测试和完整结果](evidence/web-r16-parity-goal-20261107/session-return-verification.json) · [原角色注销后选角](evidence/web-r16-parity-goal-20261107/session-return-selection-original.png) · [第四槽角色返回](evidence/web-r16-parity-goal-20261107/session-return-selection-nonfirst.png)

首次 Q 的竞态来自 HUD 导航快照：按键已同步修改 ref 和导航版本，但 React 尚未重绘，快照仍提交旧的关闭状态；后来的 Native 回投又将已打开的日记关掉。快照现读取即时 ref，覆盖 Q、背包、角色窗口和技能页。四项回归执行实际 Page setter、快照、hook 和回投，在旧实现下失败。171 项选定 Node 回归、4 项 Native 注销／选角测试和 Web TypeScript 检查通过；Native 为编译测试，窗口实机另验。[Node](evidence/web-r16-parity-goal-20261107/session-return-node-final.log) · [Native 返回选角](evidence/web-r16-parity-goal-20261107/session-return-native-return-selection.log)

Chrome 156 下先以 WebGL2-shared 完成中、英、阿六次独立冷启动，分别在世界可见及 Quest 首次 ready 时只按一次可信 Q，保持一秒后从 X 关闭。随后正常注销和选角离开，均无移动、战斗、用药或任务变更。中、阿在默认 WebGPU 又各通过一次。两个完整轮分别使用 WebGL2-shared 和默认渲染路径，实读原 30 级／26 项任务、切换到普通创建的 1 级角色、打开技能页、注销并重进检查默认页签，离开后正常重新登录原角色，各四进四出。中间返回不增加 login 或更换 socket，明确离开才换连接。[六次冷启动之一](evidence/web-r16-parity-goal-20261107/session-return-cold-chrome-ar-ready-2.json) · [阿语首次 Q](evidence/web-r16-parity-goal-20261107/session-return-first-q-ar.png) · [英文首次 Q](evidence/web-r16-parity-goal-20261107/session-return-first-q-en.png) · [WebGL2 完整轮](evidence/web-r16-parity-goal-20261107/session-return-journey-chrome-final.json) · [默认路径完整轮](evidence/web-r16-parity-goal-20261107/session-return-journey-webgpu-fixed.json)

默认 WebGPU 的首次对照虽完成四进四出，仍出现 52 次 404，对应 23 份未封装的实体／角色 shard。有限包仅保留完整索引及选定地图、原图和预构建资源，API 却仅凭索引存在和哈希宣告完整包已验证。现由发行端明确的启用、验证、哈希和路径共同授权完整包，有限索引继续参与版本元数据，默认实体路径使用已有预构建／原图回退。正向测试保留已验证远端完整包和独立地图 atlas。新增五项回归修前四项失败，修后全部通过；相关 24 项测试和 8 项 standalone tracing 通过。首次相邻测试因旧 loader 缺相对 require、未提供 trace fixture 失败，修复 loader 并指定独立 fixture 后通过，原日志保留。[修前浏览器](evidence/web-r16-parity-goal-20261107/session-return-journey-chrome-1.json) · [修前回归](evidence/web-r16-parity-goal-20261107/session-return-asset-capability-before.log) · [最终回归](evidence/web-r16-parity-goal-20261107/session-return-asset-capability-final.log) · [实际 WebGPU 包和关闭的完整包能力](evidence/web-r16-parity-goal-20261107/session-return-cold-webgpu-fixed-zh.json)

23 份浏览器记录连同失败全部保留。早期两次脚本等待隐藏的旧 canvas，后改为实际舞台焦点；旧 headless-shell 出现约 61 秒无法归属 JS 的主线程长任务，当前 Chrome 的两个后端未复现相同停顿。另有几次 Alt+X 可信按键后没有发出 logOut；加入只读诊断和最终普通源码完整轮均通过，原因尚未确定，继续列开放。不能把后续通过解释成这些失败均已修复。阿语截图仍可见既有 Native 英文夹杂，文案校对状态不变。[停顿剖析对应记录](evidence/web-r16-parity-goal-20261107/session-return-journey-perf-1.json) · [未发注销指令](evidence/web-r16-parity-goal-20261107/session-return-journey-chrome-webgl-1.json)

本轮从原在线战士服一次复制为隔离 File 副本。对不可变基线比较，原角色 44 项存档字段只有世界 NPC 事件和 revision 不同，26 项成长／资产字段及位置、朝向一致；两个原账号保留，其他账号内容一致。副本通过普通创角界面新增的三个 1 级角色保留。原三服持续在线，不声称其文件字节不变。取证后关闭本轮 19247／19277／19349，保留原三服。本轮为源码环境验证，原试玩入口、发行包和正式环境尚未替换。[比较和分阶段源码范围](evidence/web-r16-parity-goal-20261107/session-return-verification.json) · [清理](evidence/web-r16-parity-goal-20261107/session-return-cleanup-final.json)

## 稳定快捷键与 Windows 原锁构建

继续复现旧失败：第四次可信 Alt+X 已到浏览器，Shell 函数首行和 Page 入口探针却仍是上一次会话的记录，未进入游戏快捷键处理。Shell 原来随回调和界面状态变化卸载、重装 window capture listener。前序同目标监听器在浏览器回调间的微任务中提交界面更新时，这种模式可能让旧监听器被移除，而新监听器赶不上当前事件。独立 Chrome 156 的真实 DOM 对照各只按一次可信 Alt+X：旧模式调用零次，实际新 hook 调用最新处理器一次；仅 React 提交阶段用 fixture，未连接账号或 Gateway。原第四次没有完整的历史 add/remove trace，不把机制对照当成它的精确时序记录。[失败入口探针](evidence/web-r16-parity-goal-20261107/shortcut-shortcut-diag-2.json) · [真实 DOM 对照](evidence/web-r16-parity-goal-20261107/shortcut-dom-control.json)

Web 现由 `useWindowKeydown` 固定 capture 订阅，layout commit 才切换处理器，离开游戏和卸载会退休处理器。原快捷键函数逐字相同，焦点、模态、所有权、重复按键等判定完整保留。133 项选定 Node 回归和 TypeScript 通过，覆盖实际 Shell、hook、Storage、技能模态和注销；已有测试 fixture 的 useCallback 缺失也已修正。最初两次功能复测的监听器观测器只匹配精确函数名，记录为空，不能证明订阅次数；最终改为非空断言后重新取证。[源码及测试范围](evidence/web-r16-parity-goal-20261107/shortcut-verification.json) · [133 项回归](evidence/web-r16-parity-goal-20261107/shortcut-stable-keydown-node-final.log)

最终 WebGL2-shared 与默认 WebGPU 各完整四进四出，包含 30 级／26 项读回、另一角色、技能窗口、注销重进和明确离开后的重新登录。两个运行从首次进城到第四次进城均保持开发 Strict Mode 初始两次 add、一次 remove，实际存活订阅始终一个，导航中没有重绑。各四次 LogOutSuccess，网络和页面错误为零。26 项个人成长／资产、位置和朝向与隔离基线相同，44 项存档仅 NPC 事件和 revision 不同；副本一个普通创建的 1 级角色保留。[WebGL2](evidence/web-r16-parity-goal-20261107/shortcut-qualified-webgl.json) · [WebGPU](evidence/web-r16-parity-goal-20261107/shortcut-qualified-webgpu.json) · [选角截图](evidence/web-r16-parity-goal-20261107/shortcut-selection.png)

Windows 原锁未跟上共享 runtime 既有 wasm32 getrandom(js) 依赖。仅同步三个依赖边，571 个包的版本、来源和 checksum 不变，CRLF 保留。当前 canonical manifest 和该锁的 `release --offline --locked` 构建通过；608 项源／数据／资源在构建前后稳定，395 项实际编译输入均有前置 pins。新候选 SHA256 为 `bc3bde1d0f063f3c52abe790424249a983f298f4941245a5b6c2c0081dd5190a`，保留此前外置锁候选和原安装 EXE。[锁同步](evidence/web-r16-parity-goal-20261107/shortcut-native-lock-sync.json) · [原锁构建](evidence/web-r16-parity-goal-20261107/shortcut-native-canonical/native-canonical-build-result.json)

原外置锁候选仅通过测试专用自动登录收到 LoginSuccess，尚未 StartGame；桌面自动化被用户物理 Esc 中止。此后用户明确优先不控制电脑的部分，Windows 窗口点击及跨端实机暂缓，不因编译成功而计入窗口验收。停止的是本轮已核对路径和哈希、仍在选角的测试客户端。19248／19249／19278／19350 均已关闭，原三服继续在线，原试玩包及正式环境未替换。[完整边界与保全](evidence/web-r16-parity-goal-20261107/shortcut-verification.json) · [清理](evidence/web-r16-parity-goal-20261107/shortcut-cleanup-final.json)

## 组队悬停、邀请与会话

Native 的普通组队窗与快捷键只提交 AddMember；共享网关检查被邀请者是否接受邀请，成功后才打开邀请方的开关。Web 去掉普通邀请前的 SwitchGroup，并将 Ctrl+G 从已选中对象改为鼠标下的玩家。每次按键按当前 DOM、共享 UI 几何和真实鼠标位置重新判断；背包、任务、模态窗、编辑框和已按住的手势不能穿透。当前自体、死亡、隐藏及已离开 AOI 的对象不能提供邀请名字。阿洛专用入口保留 Native 已有的显式开关路径，不据此改写所有邀请入口。[源码与完整范围](evidence/web-r16-parity-goal-20261107/group-verification.json)

组队开关显示服务端实际值。发送前复核当前角色、物理连接、地图、队长及十五人上限，同名同操作不重复发送。只有匹配名字的 AddMember／DeleteMember 或完整权威名单解除待确认状态；换图保留，成功的新 StartGame 与角色会话结束才清理。发送结果未知时不自动重试，本地确定未发送则保留姓名草稿。296 项 Node、507 组适配器和限定 Web 源码 TypeScript 检查通过；旧 fixture 的依赖及发送末尾断言同步到实际源码，未删除原保护。[Node](evidence/web-r16-parity-goal-20261107/group-node-qualified.log) · [适配器](evidence/web-r16-parity-goal-20261107/group-stage5-qualified.log)

最终 Chrome 156 在 WebGL2-shared 与 WebGPU 各用一个既有一级副本及两个普通注册账号，实际验证：被邀请者关闭时拒绝且邀请方仍关闭；背包和编辑框阻挡；选中 A、悬停 B 时只邀请 B；接受后显示真实两人名单；移除和两次正常注销重进。两场均零移动、战斗、物品操作、页面和资源错误。没有使用管理员命令、传送或模型调用。[WebGL2](evidence/web-r16-parity-goal-20261107/group-browser-webgl.json) · [WebGPU](evidence/web-r16-parity-goal-20261107/group-browser-webgpu.json) · [组队](evidence/web-r16-parity-goal-20261107/group-webgpu-party-established.png) · [背包阻挡](evidence/web-r16-parity-goal-20261107/group-webgl-bag-blocked.png)

三份基线角色的 78 项成长／资产比较相同；未进入的原三十级角色及另一角色全部保存字段相同。参与组队的一级副本在首次入场已收到 `(287,617)`，与复制档的 `(288,616)` 不同，后续各场起止相同；最终差异为该坐标、NPC 事件和 revision，另行保留。新增两个账号来自普通注册。原三服继续在线、原安装 EXE 哈希保持；本轮服务已关闭，未替换试玩包或部署正式环境。[存档](evidence/web-r16-parity-goal-20261107/group-preservation.json) · [清理](evidence/web-r16-parity-goal-20261107/group-cleanup.json)

该轮自动断线恢复未通过。前两次 Playwright 路由使用浏览器 close API 不接受的 1001，错误被测试工具吞掉，不能认定真实关闭；修正为 3001 后，实际日志显示旧连接已结束，但保存耗时 7442 毫秒，新连接约一秒就发送 StartGame，在旧会话进入 resume store 前被路由租约拒绝。原失败继续保留，后续修复与复验见下节。[合法关闭失败](evidence/web-r16-parity-goal-20261107/group-reconnect-valid-close-failed.json) · [脱敏日志](evidence/web-r16-parity-goal-20261107/group-resume-sanitized-log.json)

本轮只限制组队子队列八项，尚未对齐 Native 跨所有社交操作的总八项预算。固定 Crystal 的组队拒绝走系统消息或静默限流，后续本机已复验手动重试。 [后续证据](evidence/r16-group-invite-retry-20261110/README.md)隐藏对象仅补齐组队资格，完整隐藏渲染另验。Windows 窗口、交易、阿洛、发行和真人验收仍开放，`Candidate100=false`。

## 慢保存与自动重连（2026-11-08）

同一 Gateway 进程内，WebSocket 的普通登录／StartGame 现在只等待已确认断开的旧连接。匹配同账号、角色、服务端租约持有者及唯一连接标记后，在会话单写者锁外等待；旧会话真实保存、解冻并存入恢复库后才通知。恢复仅消费一次，等待和容量准入后再次核对身份有效期、撤销与版本。失败、取消、过期及竞争消费均不降级成冷启动，原在线租约、恢复日志与保存围栏保持。[源码摘要、构建、测试和范围](evidence/web-r16-parity-goal-20261107/reconnect-verification.json)

33 项新回归覆盖通知、一次消费、身份撤销、实际 checkpoint 阻塞／失败及移动排空；相邻回归去重后合计 103 项通过。新 Gateway 为隔离的 dev/debug 候选，SHA256 `e519dc7a6d0eb3f399fcb20c1145bb260418ec0c13e3d7ad44327610fc987c55`，九项直接源码在构建前后稳定；未据此宣称完整发行源闭包或 release 构建通过。[测试](evidence/web-r16-parity-goal-20261107/reconnect-test-summary.json) · [候选构建](evidence/web-r16-parity-goal-20261107/reconnect-candidate-build.json)

Chrome 156 无界面运行 WebGL2-shared 与 WebGPU，复用既有普通账号与角色，均完成合法 3001 断线、唯一自动 StartGame 恢复旧会话、同名邀请、遮挡、接受、移除及正常注销重进。两次保存分别 7412／7728 毫秒，新请求等待 861／1009 毫秒后恢复，租约拒绝为零；整场零移动、战斗、物品及页面／网络错误，无管理员命令和模型调用。五份基线角色的 130 项成长／资产、位置及朝向相同；活跃角色仅 NPC 事件和 revision 不同，原三十级及未使用角色全部字段相同。[WebGL2](evidence/web-r16-parity-goal-20261107/reconnect-browser-webgl.json) · [WebGPU](evidence/web-r16-parity-goal-20261107/reconnect-browser-webgpu.json) · [生命周期](evidence/web-r16-parity-goal-20261107/reconnect-lifecycle.json) · [存档](evidence/web-r16-parity-goal-20261107/reconnect-preservation.json) · [组队截图](evidence/web-r16-parity-goal-20261107/reconnect-webgpu-party.png)

本轮服务已正常关闭，原三服、原 Windows EXE、旧候选和未提交工作保留；未替换当前试玩包或部署正式环境。此恢复存储只覆盖本进程 WebSocket，跨 Gateway／TCP、Native 凭证先行且旧档仍保存中的恢复、Windows 窗口及同候选跨端实机另验。组队共用未结项、交易、阿洛与发行门槛继续开放，`Candidate100=false`。[清理与原件](evidence/web-r16-parity-goal-20261107/reconnect-cleanup.json)

## 凭证先行的保存交接（2026-11-08）

Native 默认断线后以 250 毫秒基准退避，直接使用 `resumeSession` 恢复凭证，收到拒绝会停止该次自动恢复；此前网关只接受已发布的恢复档，因此旧连接仍保存时可能被过早拒绝。现只为已确认断开的同角色、同租约持有者等待保存，等待不持单写者或恢复库锁。完整凭证 binding（含 family、nonce、generation）与本次保存的连接标记在同一预约事务内比较；继续原身份、月卡、路由、Zone、一次消费及提交后共享撤权复核。[源码、范围与完整证据](evidence/r16-native-resume-wait-20261108/verification.json)

最终消费锁内再次检查新连接是否已结束；结束则沿原 RAII 恢复未消费的预约与容量。互斥锁等待跨到期原本已有清理保护，本轮另覆盖清理后到消费前的调度窗口，在消费前刷新时间。精确负测在刷新前失败、修后通过。未知／活连接不等待，失败、取消、撤权、过期和竞争消费不转为冷启动；已保存且未等待的合法凭证继续原恢复路径。Native 客户端源文件及十四秒全生命周期预算未变。[修前回归](evidence/r16-native-resume-wait-20261108/tests-pre-consume-clock.log) · [最终测试](evidence/r16-native-resume-wait-20261108/tests-qualified.log)

新增十三项存储层和两项真实 Axum WebSocket 测试通过；相邻回归去重后一百零三项通过，合计一百一十八项独立案例。真实测试用临时 File 档、普通注册／登录／创角／入城：旧 checkpoint 尚被门闩阻塞、恢复库仍空时，新 handler 已持有等待订阅；放行后恢复成功。另一案例先取消等待连接，再用同凭证恢复；两例均核对唯一代次、原凭证重放拒绝、ACK 后的有效自体快照、个人字段与坐标、无聊天命令重放、正常注销及全部连接／会话／恢复容量归零。订阅计数证明 handler 到达，不单独声称 wait Future 已被 poll。[相邻回归](evidence/r16-native-resume-wait-20261108/regression-result.json)

首次工具将 packet 字段写成 name，随后误选入城前空快照，又混用了共享 wire ID 与个人 snapshot ID；对应失败原样保留，修正测试后通过。新 dev/debug Gateway 候选 SHA256 `3666eb7e67b8c99ab45621fd5b320f5e0874869868a45f48e3a1414e3e6bd9c1`，九项直接源码在测试前与构建后稳定，未声称完整发行源闭包。原三服、原 Windows EXE 及上一候选保持，未替换试玩入口。此轮未操作桌面或调用模型；Windows 窗口、同候选跨端同档、跨 Gateway／TCP 和正式发行仍另验，`Candidate100=false`。[候选](evidence/r16-native-resume-wait-20261108/candidate-build.json) · [原件保全](evidence/r16-native-resume-wait-20261108/preservation.json)

## 公会职级普通流程（2026-11-08）

在 `068fd0bd3` 保存的职级增量上，通过普通账号登录、选角、G 入口打开公会，独立执行一次改名和单项权限切换。会长的两次选项修改不会带出其他选项命令；未保存的名字草稿在权限回执后仍保留。另一真实成员只可编辑自身和更低职级，更高职级的名字、保存和八项选项均禁用。它的修改会在会长页面通过真实广播显示，无命令重放，三职级和三成员保留；正常注销后再入场读回相同结果。[两后端、源码与命令记录](evidence/r16-guild-ranks-20261108/verification.json) · [成员权限](evidence/r16-guild-ranks-20261108/webgl-officer-hierarchy-and-authoritative-options.png) · [默认路径重登](evidence/r16-guild-ranks-20261108/webgpu-owner-final-persisted-ranks.png)

Chrome 156 的 WebGL2-shared 与默认 WebGPU 各完成十二项流程断言，均零移动／战斗／用药／拾取命令，页面、控制台和本机资源错误为零。当前 `bevy-a46905128c316656` 的六份可变资源与六份不可变副本逐项验证，八份直接前端源码摘要稳定；不声称全工作区闭包。选项、页签等选中态原先混用 `border` 与 `borderColor`，实际切换产生 React 样式冲突，现统一完整一像素边框。新鲜三十九项限定检查及限定 TS 通过；九十四项 Page 和五百零七组适配器是前一检查点结果，没有重复执行或加总。[修前错误轮](evidence/r16-guild-ranks-20261108/diagnostic-browser-failed.json) · [当前检查](evidence/r16-guild-ranks-20261108/focused-tests.log)

测试使用独立 File 副本，启动前添加三人公会，仅此初始化不是普通创建公会的证明。最初 ID 不符合三十二位十六进制格式被服务端拒绝，修正后个人账户未变。首个有效轮在第二浏览器冷进入时，会长意外回登录且缺乏当时的关闭 trace；后续加强记录与最终两轮通过，不将其定因或判已修复。另一轮已到最终重登，检查器却在注销清空 player 后读取坐标，现改在每次注销前核对实际 transform。对应原失败一并保留。[失败、范围与原图](evidence/r16-guild-ranks-20261108/failures.json)

五角色一百三十项个人字段比较中，一百二十九项精确相同；唯一变化是原本缺省的 `npc_purchase_journal` 在普通 `npcPurchaseOwner Begin` 时建立了空日志。严格检查版本、账号／角色绑定、非零三十二字节 actor 与零条采购记录，已有日志不变；这与一百二十五项成长／资产字段、五角色坐标和朝向均相同分别记录。原三服及 Windows EXE 保持，本轮服务 `19253/19283/19353` 已关，未控制桌面、调用模型或发布。[最初严格差异](evidence/r16-guild-ranks-20261108/preservation-first-differences.json) · [完整保存核对](evidence/r16-guild-ranks-20261108/preservation.json) · [服务保全](evidence/r16-guild-ranks-20261108/cleanup.json)

公会像素布局、部分混合语言与输入边界、Native 非七号增量及 ACK 字段、成员显示上限和共同社交未结预算继续开放。此轮只完成所列普通操作，不提升整体 Candidate 或 Accepted。

## Native 公会增量与请求生命周期（2026-11-08）

继续从已推送的 `85eecf4be` 核对 Windows R16。原 Native 将七号以外的成员包整体替换名册，导致上线、移除或换阶的空包清掉其它成员；普通权限 pending 又等待协议没有的 `changeType`。现在按真实零至八号／二五五号区分增量和完整名册，先验证后一次提交，保留真实成员 ID。本人换阶与权限通过 `UserInformation.realId/name` 和嵌套成员身份确认；这个入包原来是独立 typed event，未经过 `Other` 的社交消费入口，现已实际接入，不从选角槽或 AOI 对象猜身份。[源码、原始失败和完整检查](evidence/r16-native-guild-20261108/verification.json)

权限请求本地保存操作者、角色、公会／代次、职级、选项和值；最终发送前再检查当前身份、权限和有效 pending。发给服务器的仍是普通 `EditGuildMember` 四字段。只有实际七号游标在提交后新鲜、操作者与嵌套职级及目标位相符才解除；随后完整名册或元数据不能藏掉已到达游标，单独二五五号不假冒权限结果。成员换阶五号及移除三号只作精确目标状态证据，没有新增 request ID 或因果结算承诺。地图切换保留，身份／公会退休取消，权限没有额外超时解锁。

独立复核另查出两个真实遗漏：自动名册读取若在资源门槛或发送通道满时确定未发，会留下 pending；普通队列满又可能淘汰已占社交名额的意图。现用本地 typed read 保存范围与提交前游标，收到新鲜完整名册才完成，确定未发仅释放本次请求，允许下一次只读刷新；已接受的读取继续保持。真实生产分支的八槽 `SyncSender::try_send` 满载、排空后重读和已发保持有实际 bridge 测试；另有先入队再受普通输入挤压的反向用例。队列仍为二十四槽，社交仍为八槽，受保护的权限／名册／改名意图不会静默淘汰。[最终 Windows 检查](evidence/r16-native-guild-20261108/windows-guild-v2-qualified.log)

最终限定检查：Bevy社交七十项、公会一百四十三项、Windows二十九项、实际入包队列六项，共二百四十八次选定执行，去重为一百九十八项，其中六十九项新增、一百二十九项相邻。两项显式的真实素材GPU/GDI图像检查未运行，旧失败与首个经复核判为未资格的候选保留。原测试中的非二五五号伪完整表、缺少成员身份、虚构字段回声和无公会成员资格夹具已修正，拒绝错误包的负向条件仍在。[测试列表和未运行范围](evidence/r16-native-guild-20261108/verification.json)

Rust1.95使用 canonical manifest／原 lock 的 release、offline、locked 构建通过；十三项直接源码、六份 manifest／lock 与 Crystal 对照文件前后稳定，不宣称全工作区输入闭包。最终单EXE候选 `a63bd2a356bced6b95000c0e35f84e0355ea0d568dbe3aef2cd98b2f74372b0d` 已另存，需要既有R16资源，未启动、安装或替换原游戏。原三服与 Windows EXE 保持，没有本轮存档写入、模型调用、桌面控制或发布。[候选](evidence/r16-native-guild-20261108/candidate-final.json) · [保全](evidence/r16-native-guild-20261108/preservation.json)

招募／建职阶的精确本地上下文和失败回执、职级索引重编号的并发边界、Native二百成员与共享一千上限差异、Web跨社交预算及全部窗口体验继续开放。此轮不关闭当前候选跨端同档或真人验收，也不把前轮 Web 会长冷进入时意外回登录判为已修复。

## 交易取消与重复接受（2026-11-09）

Web在自己的放入/取回物品未决时，通用物品屏障与交易proof均会吞掉仍可点击的取消；Native `trade_dialog::cancel`没有该物品屏障。现在只为当前owner、地图、交易代次及物品proof放行一次取消，在最终socket尝试前才消耗资格；明确未发送可重试，发送结果未知保持未决。真实终结交易包仅退役其绑定的trade物品操作，`unlock=true`、Guild屏障和其它资产保护保持。

复核还发现重复同partner的`TradeAccept`会无条件新建Web交易，清报价并刷新取消资格；已按Native的同physical-owner/open合同保留现有报价、代次与未决绑定。真实新邀请、换partner/owner和终结后再打开仍是新交易。

最终真实Page函数/receive harness105项、相关纯社交31项、精确发送边界适配器507组及Web TypeScript通过，分target记录不合计唯一案例。隔离浏览器两场分别延迟真实Deposit/Retrieve ACK744/661ms，实际取消、UID/数量/金币及预期slot保持，旧ACK原文释放后不复活；双方正常注销与自有服务清理通过。它们是两个分项证据，旧整体false保留；网关为已pin的v22旧隔离二进制，未冒称当前源重编、当前Native窗口或公开发布。取消时存在无关Guild物品未决仍阻挡，Native完整20变体共用八项预算与失败ACK尚未完成。[本轮证据与边界](evidence/r16-web-trade-20261109/README.md) · [共用预算未结审计](evidence/r16-web-trade-20261109/social-budget-audit.md)

## 共用社交预算（2026-11-09）

Web最终发送入口现直接复用Windows的`SocialModel`准入和回执判别，共同Rust层完整映射20变体，普通GuildInfo读取升级为typed read且不双占槽。按实际Native UI保留TradeRequest/TradeReply/TradeConfirm的transient路径及拒绝/只读刷新；不把低层枚举覆盖冒称20项真实UI工作流全部验收。

预约与已发送项合计最多八个。预约先于同步action事件，readonly validate先于原domain claim，最后Native enter→取消资格消耗→唯一socket尝试；明确未发可退额，throw/未知保留。真实packet按物理socket/连接接入，Native自行持有邀请epoch、viewer/公会cursor和完成规则；同帧raw报价与严格库存投影提供自己的trade回执。原物品/权限proof和经济ACK后完整snapshot屏障保持；换图不清未决，真正会话退休才清。

Native108、runtime9、原Windows投影5、Page116/host17、Web选定569及Stage5内部507组分别通过，TS退出0；计数重叠不合并。三套完整WASM为`bevy-0cd68510886134e0`，原feature/体积门槛保持，实际Chrome每包338桥调用通过，旧a469不可变文件保持。本轮页面/运行器为本机候选，普通UI证据与pure ABI分开；未公开发布、未启动模型或改服务端/存档schema。原构建布局错误、夹具依赖错误和旧9>8反例均保留。[限定证据](evidence/r16-social-budget-20261109/README.md)

隔离旧v22 File网关和两原测试角色使用新Page/WebGPU运行包实跑。组队双方两人、未入会公会只读、真实Deposit/Retrieve成功ACK延迟后取消、主角窗口不复活及UID/数量/金币/预期格准确；双方普通注销、同角重登及最终注销均核对资产。取回原r3仍因辅助客户端当场uncertain失败，后续raw trade=null、未保留完整早期包序；不能将重登入档等同当场状态已修。[普通UI与原失败](evidence/r16-social-budget-20261109/README.md#ordinary-isolated-web-ui)

未产出的失败回执仍需核对普通协议的实际来源。Native 招募与建职阶、阿洛、语言输入、发行和真人 Accepted 继续开放。Windows PC HUD 与钱包优先 opt-in 有独立证据，Web 新外观另验。

## Gateway 交易终结与单件成交（2026-11-09）

在当前源码的隔离 File Gateway 以普通 WebSocket 双端重现原 r3。发起方仍待初始化时，受邀方先接受、编辑并取消。旧实现发出接受和报价，随后因个人交易尚未打开而缺少终结。修复将终结排到该对端原接受及报价队列之后，已打开交易仍走原退回和协调。新回归先红后绿，修复版两种节拍、放入与取回取消、同连接再开和同角读档通过。

新 WebGPU 页面连精确修复版，延迟真实放入回包 788 ms、取回回包 599 ms，各自点击取消。双方交易关闭，迟到回包未重新打开，资产正确，双方正常退出。独立普通协议完成一次蜡烛交割，真实入账后双方收到完成回包并同角读档。第一轮交割探针误用发送方标识的失败保留。Gateway 汇报 64 项通过，三项 PostgreSQL 用例未实际访问数据库。Node 通过 15 项。界面仍可见中英混排。未部署公开版本，全局 Candidate 与真人 Accepted 继续开放。 [回归、普通包序与界面证据](evidence/r16-trade-terminal-20261109/README.md)

## 公会仓库待回包时取消交易（2026-11-09）

Windows R16 为开着的交易独立排队取消。Web 原按钮可点击但发送门禁吞掉取消，已在真实会籍和存入回包暂缓的旧页面复现。修复仅允许当前已开交易独立取消，保留当前角色、地图、交易代次、单次发送和预算限制。修复版 WebGPU 存入和取回各发一次取消，双方各收到结束包，仓库仍锁到匹配回包与后续完整快照。两次普通同角读回加最终 File 校验通过。149 项 Web 回归与 TypeScript 通过。仓库全量按 Crystal 每个公会会话一次，探针中人工重排回包及错误的重复读列表断言均保留。 [仓库和交易双端实测](evidence/r16-guild-trade-cancel-20261109/README.md)

## 双向交割与暂存界面（2026-11-09）

第一人确认后，R16 共用 Gateway 将物品移出背包暂存。旧 Web 查背包失败后丢报价与锁定，已修复为按当前交易的权威暂存快照显示原确认报价。两普通 WebSocket 与双 WebGPU 实跑蜡烛和木剑互换、137 与 59 金币互付，双方各一次入账后结束，最终钱包 922／778，重登及落盘精确。该版本暂存后解锁关闭并退回资产，11 月 10 日复测已能恢复原交易。原两次冷双开的登录异常保留，本节双浏览器正向结果来自预热服务。 [历史双向交割](evidence/r16-bidirectional-trade-20261109/README.md) · [解锁复测](evidence/r16-trade-reopen-20261110/README.md) · [生产冷双开](evidence/r16-guild-cold-entry-20261110/README.md)

## 阿洛体验自述与用量费用（2026-11-09）

Web 按 Native 培养和行动页补齐体验自述、选择理由、教学理解以及用量和费用。保持当前角色私有会话，服务意向采用账本的当前体验与报价。普通 Chrome 从登录进城到固定教学、体验记录及实际导出通过。下载档案通过 SHA-256 与 HMAC 核验，停止并重启规则运行器后，同一阿洛的身份、v2 三项习惯、原教学与体验记录仍可在 Web 查看。 [截图、收据和边界](evidence/r16-alo-trial-web-20261109/README.md)

下载档案又导入新的 SQLite 并由新进程运行。阿洛通过 Web 普通邀请入队，跟随主人移动 7 格，并在天然鹿死亡前和主人各实际命中 6 次。体验自述页改为读回服务器确认的当前来源和次数，关页重开仍显示 1 次。 [实测和未验项](evidence/r16-alo-trial-web-20261109/README.md#新实例导入与普通协作)

战斗掉落对照发现旧式本地库存编号冲突会拒绝普通拾取。修复版仅在共享本地领取时为冲突的进入副本选用空闲编号，普通联机入包和重登通过。Web 实战证实 v1 在鹿死亡前捡到蜡烛，随后固定教学及同一身份重启读回 v2 通过。第二场未取得玩家命中，v2 战后拾取顺序仍开放。 [原拒绝、修复与实战](evidence/r16-alo-loot-order-20261109/README.md)

## 当前执行顺序

2026-11-09：用户重新激活完整Web/Windows R16对齐Goal。此前发布任务已完成：Web f2de、Linux d53、固定42738素材、新Numeron域名、旧129账号/43角色/43档保全、公开三职业首任务/重登及实际中文Q进城均有分源证据；见[发布结果](NUMERON-WEB-RELEASE-20261108.zh-CN.md)。本轮从已推送`6d30e0f9e`继续，旧正式服/Windows EXE/现有存档保留；不得据此勾完剩余范围或把TEST称为主网。

用户要求优先提交推送的代码检查点已保存；后续公会普通流程的限定资格见上节，仍保留原检查点时“浏览器未验”的历史范围。[原保存范围](CODE-CHECKPOINT-20261108.zh-CN.md)

1. 已在真实存档副本完成小地图标题及框架输入修复、缩放和前景 Q 日记复测。
2. 已完成 Web 创角回执、共享网关四槽和重名、登录过场输入及标签的本机验证。Windows 窗口和超额旧档访问继续追踪。
3. 已将三职业毕业装备建议和 D605 的既有 Native 名称接进共用日记，完成三职业读档、十语窄窗、65 次关闭零移动与存档个人字段比较。
4. 本方确认后加价、交易邀请反馈、组队名字回执及拒绝或静默后的手动重试已有本机证据。继续公会权限及新运行包的发行复验。阿洛跨端读写已验，继续跨端实战、语言和完整离线生活。
5. 用普通协议和无界面浏览器继续核对技能施放、地图及场景叠放。Windows 窗口实机、旧 headless 长任务、正式发行和最终真人验收单列。
