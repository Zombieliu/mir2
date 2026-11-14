# Web / Windows 有界行为矩阵07

## Source21：两tooltip源码候选闭合（2026-10-07）

Source21为实际自有Bag/Belt/装备提示接入共享Rust纯formatter、neutral DTO/name表和renderer-free可选PUI ABI1；Native四个公共API与默认ShiftClick选项保留，Web按真实MoreActions入口展示拆分提示。完整document含11类sections/10种colours、原始/真实/解析/socket属性与unknown viewer；raw UID0、signed i32 item index、count/dura/unit sale及完整carrier从实际显示对象独立克隆，catalog/label/slot不能补实例来源。ABI getter custody、canonical i64 .NET ticks与exact formatter clock缓存保持；缺可选能力不阻断普通Core接口，缺字段/partial不伪装完整。

真实Page reader在useLayoutEffect提交后才格式化，调用前后核验物理owner/socket/connection/session/scene/map、Core对象、来源签名、window/tab/epoch和raw对象唯一性；Bag/Belt/Character真实mount传入相应callback，hover/focus/touch只读。active-only1000ms刷新，blur/visibility/pointercancel/resize/pagehide/unmount及旧cleanup不能清掉successor；原basic fallback保留。movement-only itemIdentity纳入sellValue，金币售价单项变化能刷新提示。本批不产生装备/使用/修理/发送权限。

Root选定Rust跨配置169次通过：Core101＋default44（另3项原ignored）＋Native3＋finalPUI21。最终PUI hint修正后只有21新执行，其余148为本次Source21先前实际执行，仅按未变有效配置输入限定承接；不称169都在最终修正后跑过。18项新增distinct、30项tooltip/name执行，原12个Native具名tooltip测试保留（11迁到Core、1留Native）。现有11个Node脚本Node03实际218/218、0失败/0跳过，Stage5原290＋9新增＝299内部组只计一个文件级Node测试，不能218＋299。原clock组名称/断言强化exact formatter clock，旧负index拒绝替换为below-i32拒绝＋合法negative index覆盖；实际组件唯一button AST selector收窄，保留card count1与六events检查，不声称全部旧断言字节不变。Combat23原测试/断言保持。strict code-only非增量TSC03通过。Node01的217/218及失败日志、初始PUI20、旧prepared/build配置完整保留；source-review旧pending句只表示创建时态，最终回执更正当前状态。

当前Core/PUI实际build02通过：Core WASM252417 B/JS24393 B，PUI WASM211995 B/JS16829 B，均满足原WASM<262144 B及JS≤204800 B预算；Core版本cdbc1b7c…、PUI23059247…、source49a8ea7b…，完整hash见组合证据。三renderer实际build02通过原预算，发布版本bevy-e9f57ef01a6282c7。Native EXE实际build01为104760320 B/SHA256`0b54aee401aa2504939dd3dbae80615f597ec83ae19da1321e7170ff43baaf30`；最终rust02仅改PUI，Native有效输入未变，按限定条件承接该实构建，不另称第二次Native构建；EXE未启动。既有.gitignore将Renderer immutable release列为生成资源，未强制入Git；selected Core/PUI四叶提交由Root完成，文档写入不证明发布。

Next04实际退出0（PID101008，142707ms），dist`.next-web-windows-catchup-20261007-04`，strict TypeScript21.9s与13静态页；27404声明输入，19007文件/308目录含根/603079321 B；61 NFT/37094引用（37092文件＋2目录），0missing/private，声明junction未遍历，仅原允许的生成元数据更新。Thin04实际退出0（PID82236，25804ms），包`.mir2-thin-client-web-windows-catchup-20261007-04`，63153输入；7301文件/776目录含根/0links/372903563 B，原377487360 B cap余4583797 B。独立purefs复核63153inputs/19007Nextfiles/7301Thinfiles与11 runtime copies全部一致，0确认blocker、0执行/0写；44 warning完整多重集保持，231JSON实际source/output pairs一致，仅限定承接历史token结论，未新跑parser/lexer。

仅`F05.bag.tooltip-compat`（含装备）与`F05.belt.tooltip`两行open→legacy/sourceCandidate closed。共享formatter不将整个DOM host升级为shared controller；当前103 shared/202 legacy/4 open/8 common limitation。其余315整row、317 ordered IDs、全部originalAudit/native和历史rawCounts保持，player全部not-run；实际row有限字段展平254关联/121唯一声明，每row去重后253关联；F01.safe-key.open原Overlay声明有1处行内重复并保留。旧Matrix06 summary230/118＋新增18/9＝248/127仅为历史summary算术，不是fresh当前总数；旧06实际字段展平236/112。关联数不等于执行、Node218、Stage5299内部组或玩家验收数量。317条只是有界审计记录，没有完整验收分母或总体百分比，Candidate100=false、goal active；源码/CPU有限检查/静态构建不证明可玩。

Source20已实际提交push`463d6f1ce42fefdd4244cbd801f92715e29674a1`；Source21 commit/push在证据创建时仍pending，由Root最终实际报告。下一项Source22真实Bag→Belt，随后`F02.world.fishing-click`、`F09.NPC.PEARL`、`F11.RANKING.INSPECT`，这4项raw open不是总体分母。W6实际UI、HTTP/socket、WASMAPI/实例、登录/战斗/保存重登、移动真机及最终frontend验收仍未运行，用户暂缓界面操作持续有效。

固定Windows基线`f72e36fb84c3574fff0aeb2abed856454b14289d`，Source21父提交`463d6f1ce42fefdd4244cbd801f92715e29674a1`。证据：[有限结果](tooltip-source21-finite-result01.json)、[动作链复核](tooltip-source21-action-chain-review01.json)、[组合构建](tooltip-source21-combined-build-result01.json)、[完整矩阵JSON](feature-matrix07.json)、[Current Evidence07](current-evidence07.json)。[矩阵06](feature-matrix06.md)和更旧矩阵完整保留历史；其两tooltip旧“local props/no shared getter”结论已由Source21当前源码补充更正。

## 两条当前tooltip行

| ID | 当前源码状态 | 玩家状态 |
| --- | --- | --- |
| `F05.bag.tooltip-compat` | legacy/sourceCandidate closed，实际Bag与装备完整来源、共享getter、真实hover/focus/touch提示 | not-run |
| `F05.belt.tooltip` | legacy/sourceCandidate closed，实际Belt实例完整来源、共享document与active-only刷新 | not-run |

## 具名有限检查关联

按row有限字段declarations或兼容declaration数组展平，identity用path＋name/declaration：Matrix06实际236关联/112声明，Matrix07实际254关联/121唯一声明；每row去重后253关联。F01.safe-key.open保留的Overlay声明行内重复1次。新增9声明各关联两条共同tooltip行，共18关联/9声明。旧Matrix06 summary230/118＋18/9＝248/127只是历史summary算术，不能作为fresh当前总数。共享fixture映射不代表独立执行或每行全部验收，不能与Node218或Stage5299内部组相加。

| Stage5声明（实际源行） | 关联稳定ID |
| --- | --- |
| PUI optional item formatter preserves capability independence exact clock cache and ABI getter custody（5099） | `F05.bag.tooltip-compat`、`F05.belt.tooltip` |
| Shared item clock uses exact canonical i64 ticks and bounded caches while signed catalog indexes remain real（5126） | `F05.bag.tooltip-compat`、`F05.belt.tooltip` |
| Core actual item reader facade proxies only presentation and leaves missing optional capabilities usable（5141） | `F05.bag.tooltip-compat`、`F05.belt.tooltip` |
| Page owned instance request preserves UID zero signed index unit sale value count and independent raw carrier（5149） | `F05.bag.tooltip-compat`、`F05.belt.tooltip` |
| Page actual owned readers bind displayed Bag Belt Equipment objects and commit source before any format read（5165） | `F05.bag.tooltip-compat`、`F05.belt.tooltip` |
| Page actual owned readers reject physical owner source visibility and reentrant changes before and after Core（5184） | `F05.bag.tooltip-compat`、`F05.belt.tooltip` |
| Actual Bag Belt Equipment tooltip events are read only and active timer cleans stale source and successor safely（5244） | `F05.bag.tooltip-compat`、`F05.belt.tooltip` |
| Three actual tooltip mounts carry exact reader props document and original fallback without raw bulk actions（5289） | `F05.bag.tooltip-compat`、`F05.belt.tooltip` |
| Actual movement snapshot item projection keeps sale value changes out of confirmed movement fast path（5323） | `F05.bag.tooltip-compat`、`F05.belt.tooltip` |

## 当前记录集剩余代码差距

- `F02.world.fishing-click`
- `F05.belt.move-from-bag`
- `F09.NPC.PEARL`
- `F11.RANKING.INSPECT`

## 全部稳定记录

| ID | 玩家行为 | 当前源码类别 | 玩家验证 |
| --- | --- | --- | --- |
| `F01.login.credentials` | 输入账号/密码并提交登录 | legacy | not-run |
| `F01.login.focus` | 账号/密码焦点、Tab/Enter 输入 | legacy | not-run |
| `F01.register.submit` | 创建账号并等待服务器结果 | legacy | not-run |
| `F01.register.profile` | 填写确认密码/真实姓名/生日/密保问题答案/邮箱 | legacy | not-run |
| `F01.password.change` | 修改账号密码：旧密码、新密码、确认、结果/封禁 | legacy | not-run |
| `F01.safe-key.open` | 打开随机虚拟键盘并选择账号/密码字段 | legacy | not-run |
| `F01.safe-key.press` | 从真实随机键盘追加字符 | legacy | not-run |
| `F01.safe-key.delete` | 删除虚拟键盘当前字段末字符 | legacy | not-run |
| `F01.safe-key.random` | 重新随机排列虚拟键盘 | legacy | not-run |
| `F01.safe-key.enter` | 虚拟键盘 Enter 发起普通密码登录 | legacy | not-run |
| `F01.character.select` | 选择服务器返回的角色槽位/角色身份 | legacy | not-run |
| `F01.character.create.name` | 创建角色时填写名字 | legacy | not-run |
| `F01.character.create.class` | 创建角色时选择职业 | legacy | not-run |
| `F01.character.create.gender` | 创建角色时选择性别 | legacy | not-run |
| `F01.character.create.submit` | 提交创建角色/服务器反馈后更新列表 | legacy | not-run |
| `F01.character.delete.confirm` | 删除所选角色并明确确认 | legacy | not-run |
| `F01.character.delete.cancel` | 取消删除角色 | legacy | not-run |
| `F01.character.start` | 进入所选角色，等待真实 StartGame | legacy | not-run |
| `F01.select.credits` | 点击角色选择 Credits | common limitation | not-run |
| `F01.login.cancel` | 关闭登录/退出角色选择 | legacy | not-run |
| `F02.world.walk` | 左键按住普通行走/放开停止 | legacy | not-run |
| `F02.world.run` | 右键按住跑动，站立起步降级行走/中间格碰撞 | legacy | not-run |
| `F02.world.newmove` | NewMove 右键目的地路径，设置关闭撤销旧路径 | legacy | not-run |
| `F02.world.keyboard-move` | 未被快捷键占用的 WASD/方向移动与 Shift 跑 | legacy | not-run |
| `F02.world.turn` | 走到同格/方向输入只转向 | legacy | not-run |
| `F02.world.target-monster` | 点选/锁定真实怪物并追击 | shared | not-run |
| `F02.world.npc` | 点击实际附近 NPC 打开服务器对话 | legacy | not-run |
| `F02.world.harvest` | Alt 左键尸体/资源或空格方向采集，按住重试有节流 | shared | not-run |
| `F02.world.pick-object` | 拾取实际掉落物 objectId | legacy | not-run |
| `F02.world.pick-tile` | 当前脚下格拾取 | legacy | not-run |
| `F02.map.big-image-route` | 大地图图像任意合法目的格发起普通寻路 | legacy | not-run |
| `F02.map.mini-image-route` | 小地图已绘制 crop 点击合法目的格寻路 | legacy | not-run |
| `F02.map.transfer` | 经过实际入口换图，退役旧 map/route/target 输入 | shared | not-run |
| `F02.world.lifecycle` | UI起源、跨窗、pointerup/cancel/lostcapture/blur 不泄漏移动 | shared | not-run |
| `F02.world.fishing-click` | 装备钓竿时水方向真实世界点击：转向、水格检查、投竿 | open | not-run |
| `F03.quest.open` | 打开/关闭任务日志 | shared | not-run |
| `F03.quest.filter` | 选择任务阶段筛选 | shared | not-run |
| `F03.quest.group` | 折叠/展开任务分组 | shared | not-run |
| `F03.quest.select` | 选择任务并打开详情 | shared | not-run |
| `F03.quest.pages` | 任务日志上一页/下一页 | shared | not-run |
| `F03.quest.detail-scroll` | 任务/NPC 信息滚动 | shared | not-run |
| `F03.quest.guided-tab` | 任务引导/已完成/毕业页切换 | shared | not-run |
| `F03.quest.primary` | 设为主任务/引导目标 | shared | not-run |
| `F03.quest.accept` | 详情接取任务并等待结果 | shared | not-run |
| `F03.quest.npc-accept` | 当前真实 NPC 列表接取任务 | shared | not-run |
| `F03.quest.reward-choice` | 选择精确奖励 index | shared | not-run |
| `F03.quest.npc-reward-choice` | NPC 列表选择精确奖励 index | shared | not-run |
| `F03.quest.finish` | 交付完成任务及所选奖励 | shared | not-run |
| `F03.quest.prepare-finish` | 前往交付：导航真实交付 NPC | shared | not-run |
| `F03.quest.track` | 追踪/取消追踪任务 | shared | not-run |
| `F03.quest.share` | 分享活跃任务 | shared | not-run |
| `F03.quest.abandon` | 打开放弃任务确认 | shared | not-run |
| `F03.quest.abandon-confirm` | 明确确认放弃并等待真实结果 | shared | not-run |
| `F03.quest.abandon-cancel` | 取消放弃并保留任务 | shared | not-run |
| `F03.quest.help` | 任务窗口帮助/反馈 | shared | not-run |
| `F03.world.ToggleSupplies` | 打开/关闭补给检查面板 | shared | not-run |
| `F03.world.SelectSupplyVendor` | 选择真实补给商人 | shared | not-run |
| `F03.world.ShowSupplyInventory` | 查看补给实际库存 | shared | not-run |
| `F03.world.OpenDestinationMap` | 打开任务目的地地图 | shared | not-run |
| `F03.world.NavigateQuestRoute` | 按入口/狩猎区域/补给实际路由导航 | shared | not-run |
| `F03.world.AttackTarget` | 普通目标攻击快捷入口 | shared | not-run |
| `F03.world.AttackQuestTarget` | 当前任务怪物攻击快捷入口 | shared | not-run |
| `F03.world.PickUpObject` | 任务面板指定 objectId 拾取 | shared | not-run |
| `F03.world.PickUpTile` | 任务面板当前格拾取 | shared | not-run |
| `F03.quest.route-retirement` | 换图/换角色/新意图/阻塞后撤销旧路线而不回落旧 A* | shared | not-run |
| `F03.npc.links` | NPC 真实服务链接/返回/关闭 | shared | not-run |
| `F03.npc.input` | 需要文本输入的 NPC 服务回复 | legacy | not-run |
| `F03.input.lifecycle` | 九动作持续 held 新控件不借旧按下；终端先释放旧 Quest lease | shared | not-run |
| `F04.hud.hp-mp` | 实时 HP/MP orb 与文本 | shared | not-run |
| `F04.hud.hp-view` | HPView true compact / false alternate 四 label 同帧切换 | shared | not-run |
| `F04.hud.experience` | 经验比例/数值 HUD | shared | not-run |
| `F04.hud.weight` | 负重/最大负重 HUD | shared | not-run |
| `F04.hud.identity-gold` | 角色等级/名字/金币 HUD | shared | not-run |
| `F04.hud.navigate.character` | HUD 打开角色 | shared | not-run |
| `F04.hud.navigate.bag` | HUD 打开背包 | shared | not-run |
| `F04.hud.navigate.skills` | HUD 打开技能 | shared | not-run |
| `F04.hud.navigate.quest` | HUD 打开任务 | shared | not-run |
| `F04.hud.navigate.menu` | HUD 打开菜单 | shared | not-run |
| `F04.chat.send` | 输入聊天文字/Enter 发送服务器 Chat | legacy | not-run |
| `F04.chat.channel.all` | 选择 All 聊天频道并保留真实前缀 | legacy | not-run |
| `F04.chat.channel.shout` | 选择 Shout 聊天频道并保留真实前缀 | legacy | not-run |
| `F04.chat.channel.whisper` | 选择 Whisper 聊天频道并保留真实前缀 | legacy | not-run |
| `F04.chat.channel.lover` | 选择 Lover 聊天频道并保留真实前缀 | legacy | not-run |
| `F04.chat.channel.mentor` | 选择 Mentor 聊天频道并保留真实前缀 | legacy | not-run |
| `F04.chat.channel.group` | 选择 Group 聊天频道并保留真实前缀 | legacy | not-run |
| `F04.chat.channel.guild` | 选择 Guild 聊天频道并保留真实前缀 | legacy | not-run |
| `F04.chat.history.home` | 滚到历史起点 | legacy | not-run |
| `F04.chat.history.up` | 历史向上滚一行 | legacy | not-run |
| `F04.chat.history.down` | 历史向下滚一行 | legacy | not-run |
| `F04.chat.history.end` | 滚到最新历史 | legacy | not-run |
| `F04.chat.history.drag` | 拖动 PositionBar 滚动历史 | legacy | not-run |
| `F04.chat.resize` | 聊天窗口循环小/中/大 4/7/11 行 | legacy | not-run |
| `F04.chat.settings.filter` | 聊天可见频道过滤设置 | legacy | not-run |
| `F04.chat.settings.transparent` | 聊天透明背景设置 | legacy | not-run |
| `F04.chat.settings.apply` | 应用聊天设置 draft | legacy | not-run |
| `F04.chat.settings.cancel` | 取消聊天设置并恢复已应用值 | legacy | not-run |
| `F04.chat.settings.defaults` | 恢复聊天设置默认值 | legacy | not-run |
| `F04.minimap.markers` | 小地图实际玩家/NPC/怪物点、坐标、地图名、昼夜图标 | legacy | not-run |
| `F04.minimap.collapse` | 小地图折叠/展开并保持真实地图资源条件 | legacy | not-run |
| `F04.minimap.big-map` | 小地图入口打开/关闭大地图 | legacy | not-run |
| `F05.bag.page.bag1` | 打开第一页背包真实40格 | shared | not-run |
| `F05.bag.page.bag2` | 切换第二背包并按真实扩容容量限制 | shared | not-run |
| `F05.bag.page.quest` | 切换任务物品页 | shared | not-run |
| `F05.bag.inspect` | 点物品格显示当前真实UID的菜单/详情 | legacy | not-run |
| `F05.bag.use` | 使用真实背包实例，medicine/equipment分类按共享规则 | shared | not-run |
| `F05.bag.equip` | 从背包装备到精确装备槽位 | shared | not-run |
| `F05.character.remove` | 卸下真实装备UID进入实际可用背包格 | shared | not-run |
| `F05.bag.move` | 移到真实空格/交换占用格，包括跨页 | shared | not-run |
| `F05.bag.merge` | 合并相同实例模板的合法堆叠 | legacy | not-run |
| `F05.bag.split` | 输入数量拆分堆叠，保留剩余数量 | legacy | not-run |
| `F05.bag.drop-confirm` | 丢弃实际物品，明确确认/数量检查 | legacy | not-run |
| `F05.bag.drop-cancel` | 取消丢弃/取消物品操作 | legacy | not-run |
| `F05.bag.delete-mode` | 切换delete模式、真实绑定选择、确认或取消 | legacy | not-run |
| `F05.bag.tooltip-shared` | 共享 Bag/装备 hover 完整 Native item 文档 | shared | not-run |
| `F05.bag.tooltip-compat` | React/lean 兼容 Bag/装备 hover 完整实例属性文档 | legacy | not-run |
| `F05.belt.use` | 按6个可见腰带槽鼠标/键盘使用真实item | legacy | not-run |
| `F05.belt.rotate` | 腰带横/竖旋转 | legacy | not-run |
| `F05.belt.close-toggle` | 关闭/按绑定重新显示腰带 | legacy | not-run |
| `F05.belt.move-from-bag` | 背包drag落真实腰带槽含空槽 | open | not-run |
| `F05.belt.tooltip` | 腰带实际实例完整 Native tooltip | legacy | not-run |
| `F05.item.custody-lifecycle` | UID/owner/model变更与Mail/Storage/NPC等 custody 时拒绝旧操作 | shared | not-run |
| `F06.character.equipment` | 查看人物纸娃娃/真实装备槽位 | shared | not-run |
| `F06.character.stats1` | 查看真实攻击防御等 Stats1 行 | shared | not-run |
| `F06.character.stats2` | 查看经验/负重/状态等 Stats2 行 | shared | not-run |
| `F06.skills.page` | 技能页真实学会列表/信息/经验显示 | shared | not-run |
| `F06.skills.pagination` | 技能列表上一页/下一页 | shared | not-run |
| `F06.skills.select` | 选择真实技能进入热键分配窗 | shared | not-run |
| `F06.skills.assign` | 分配1..16真实快捷槽位并等待精确 MagicKey ACK | shared | not-run |
| `F06.skills.clear` | 清除技能绑定到0，不转成另一技能 | shared | not-run |
| `F06.skills.shortcut-cast` | F1..F8/第二栏1..8等16槽基于真实hotkey施法 | shared | not-run |
| `F06.skills.target-lock` | 真实目标锁定键，施法目标与鼠标落点区分 | shared | not-run |
| `F06.skillbar.cast` | 同格按下/释放技能栏，绑定source/slot/skill/pointer一次施法 | shared | not-run |
| `F06.skillbar.drag` | 独立拖动两栏并按真实屏幕界限保存位置 | legacy | not-run |
| `F06.skillbar.cooldown` | 技能栏图标/真实冷却剩余显示 | shared | not-run |
| `F06.skillbar.refresh` | 点击技能栏 Refresh | common limitation | not-run |
| `F06.combat.melee` | 真实目标普通近战/方向攻击/追击与攻击节流 | shared | not-run |
| `F06.combat.cooldown-ack` | 实际 MagicCast/MagicDelay 驱动真实冷却起点 | shared | not-run |
| `F06.combat.health` | 受击真实怪物生命条及过期，不把快照当新命中 | legacy | not-run |
| `F06.combat.damage` | 真实DamageIndicator hit/miss/crit/heal浮字 | legacy | not-run |
| `F06.combat.revive` | 死亡且可复活时 TownRevive | legacy | not-run |
| `F06.feedback.names` | NameView关闭仍保留 hovered 与真实 quest target 例外 | shared | not-run |
| `F07.MAIL.OPEN` | 打开/关闭邮箱 | shared | not-run |
| `F07.MAIL.PAGE` | 前后翻页邮件列表 | shared | not-run |
| `F07.MAIL.SELECT` | 选择邮件/打开信件或包裹 reader | shared | not-run |
| `F07.MAIL.REPLY` | 回复已选邮件的原发送人 | shared | not-run |
| `F07.MAIL.DELETE` | 删邮件及确认/取消 | shared | not-run |
| `F07.MAIL.LOCK` | 信件锁定/解锁 | shared | not-run |
| `F07.MAIL.COLLECT` | 领取包裹物品与金币 | shared | not-run |
| `F07.MAIL.LETTER` | 新建信件并输入/确认收件人 | shared | not-run |
| `F07.MAIL.PARCEL` | 新建包裹并保留独立草稿 | shared | not-run |
| `F07.MAIL.EDIT` | 编辑正文/键盘选区及剪贴板 | shared | not-run |
| `F07.MAIL.GOLD` | 包裹金币提示/确认/取消 | shared | not-run |
| `F07.MAIL.STAMP` | 邮票切换一格/五格附件 | shared | not-run |
| `F07.MAIL.ATTACH` | 附加/移除实际 UID 物品并锁定 | shared | not-run |
| `F07.MAIL.COST` | 精确当前草稿询价 | shared | not-run |
| `F07.MAIL.SEND` | 显式发送信件/包裹及不明结果屏障 | shared | not-run |
| `F07.MAIL.CANCEL` | 取消/关闭 compose 释放未发送附件锁 | shared | not-run |
| `F08.STORAGE.OPEN` | NPCStorage打开/关闭同服务仓库 | shared | not-run |
| `F08.STORAGE.PAGE` | Bag/仓库切 pane 与翻页 | shared | not-run |
| `F08.STORAGE.DEPOSIT` | Bag真实格存入当前空仓库格 | shared | not-run |
| `F08.STORAGE.WITHDRAW` | 仓库真实格取回当前Bag空格 | shared | not-run |
| `F08.STORAGE.MOVE` | 同容器物品移动 | shared | not-run |
| `F08.STORAGE.MERGE` | 同/跨容器兼容堆叠合并 | shared | not-run |
| `F08.STORAGE.UNLOCK` | 输入密码解锁仓库 | legacy | not-run |
| `F08.STORAGE.PASSWORD_SET` | 设置新密码/确认两次 | legacy | not-run |
| `F08.STORAGE.PASSWORD_CHANGE` | 确认更改→旧密码→新密码→再次确认 | legacy | not-run |
| `F08.STORAGE.PASSWORD_CANCEL` | 取消强制密码提示并结束服务 | legacy | not-run |
| `F08.STORAGE.RENT_OPEN` | 打开新租/续租十天1M确认，不提前发送 | shared | not-run |
| `F08.STORAGE.RENT_CONFIRM` | 显式确认新租/续租、余额/服务末端复核 | shared | not-run |
| `F08.STORAGE.RENT_CANCEL` | 取消旧租赁提示不取消新提示 | shared | not-run |
| `F09.NPC.OPEN` | 普通 Gold 商店服务打开/关闭 | shared | not-run |
| `F09.NPC.SELECT` | 选择目录真实商品 | shared | not-run |
| `F09.NPC.QUANTITY` | 普通商品数量加减/上限 | shared | not-run |
| `F09.NPC.PAGE` | 商品列表前后页/滚动 | shared | not-run |
| `F09.NPC.BUY` | 普通无限 Gold 商品捕获报价后单次购买 | shared | not-run |
| `F09.NPC.RESALE` | 非普通Gold/Resale目录保留旧购买路线 | legacy | not-run |
| `F09.NPC.SELL` | 选择实际Bag物品和数量出售 | legacy | not-run |
| `F09.NPC.SELL_HANDOFF` | 共享 Buy surface 交接 Sell 页 | shared | not-run |
| `F09.NPC.REPAIR_EQUIP` | 维修装备栏已有物品 | legacy | not-run |
| `F09.NPC.SREPAIR_EQUIP` | 特殊维修装备栏已有物品 | legacy | not-run |
| `F09.NPC.PEARL` | Pearl 商品目录及珍珠余额购买 | open | not-run |
| `F09.NPC.REPAIR_BAG` | 把Bag物品拖入普通维修目标并确认 | legacy | not-run |
| `F09.NPC.SREPAIR_BAG` | 把Bag物品拖入特殊维修目标并确认 | legacy | not-run |
| `F09.NPC.REPAIR_QUOTE` | 维修真实报价/持有目标连续选择 | legacy | not-run |
| `F09.CASH.OPEN` | Cash商店打开/关闭 | legacy | not-run |
| `F09.CASH.FILTER_CLASS` | Cash按真实class过滤 | legacy | not-run |
| `F09.CASH.FILTER_SECTION` | Cash按New/Hot/All分类 | legacy | not-run |
| `F09.CASH.FILTER_CATEGORY` | Cash类别目录及滚动 | legacy | not-run |
| `F09.CASH.SEARCH` | Cash搜索商品 | legacy | not-run |
| `F09.CASH.PAGE` | Cash目录前后页 | legacy | not-run |
| `F09.CASH.QUANTITY` | Cash单商品数量/真实stock限制 | legacy | not-run |
| `F09.CASH.PAYMENT` | Cash Gold/Credit分别选择 | legacy | not-run |
| `F09.CASH.CONFIRM` | Cash购买确认和取消 | legacy | not-run |
| `F09.CASH.UNKNOWN` | Cash未知发送结果不自动重放 | legacy | not-run |
| `F09.CASH.TOOLTIP` | Cash真实ItemInfo预览 tooltip | legacy | not-run |
| `F09.CASH.PREVIEW` | Cash原角色真实sprite试穿图层 | legacy | not-run |
| `F09.CASH.PREVIEW_TURN` | Cash试穿角色左右转/关闭 | legacy | not-run |
| `F10.OPTIONS.SKILL_MODE` | 技能键模式 | legacy | not-run |
| `F10.OPTIONS.SKILL_BAR` | 技能栏显示 | legacy | not-run |
| `F10.OPTIONS.EFFECT` | 特效显示 | legacy | not-run |
| `F10.OPTIONS.DROP_VIEW` | 掉落名称显示 | legacy | not-run |
| `F10.OPTIONS.NAME_VIEW` | 名称显示 | legacy | not-run |
| `F10.OPTIONS.HP_VIEW` | 切换 HUD compact/alternate HP/MP 四标签；保留 orbs，不隐藏世界生命条 | legacy | not-run |
| `F10.OPTIONS.NEW_MOVE` | 原版新移动选项 | legacy | not-run |
| `F10.OPTIONS.OPEN` | 打开/关闭 Options | legacy | not-run |
| `F10.HELP.OPEN` | 打开/关闭 Help | legacy | not-run |
| `F10.HELP.PREVIOUS` | Help 上一页 | legacy | not-run |
| `F10.HELP.NEXT` | Help 下一页 | legacy | not-run |
| `F10.HELP.DRAG` | 拖动 Help 并保持独立位置 | legacy | not-run |
| `F10.OPTIONS.MUSIC` | 调节音乐音量 | legacy | not-run |
| `F10.OPTIONS.SOUND` | 调节音效音量 | legacy | not-run |
| `F10.OPTIONS.OBSERVE` | 切换允许观察并请求服务器确认 | legacy | not-run |
| `F10.KEYBOARD.OPEN` | 打开键盘设置 | legacy | not-run |
| `F10.KEYBOARD.BIND` | 捕获单个功能的新按键及修饰键 | legacy | not-run |
| `F10.KEYBOARD.CLEAR` | 清除功能绑定 | legacy | not-run |
| `F10.KEYBOARD.RESET` | 恢复原版96项默认绑定 | legacy | not-run |
| `F10.KEYBOARD.ENFORCE` | 切换强制绑定规则 | legacy | not-run |
| `F10.KEYBOARD.CLOSE_SAVE` | 关闭并持久化绑定 | legacy | not-run |
| `F10.KEYBOARD.CAPTURE_BLUR` | 失焦退出按键捕获 | legacy | not-run |
| `F11.GROUP.INVITE` | 具名邀请组队 | legacy | not-run |
| `F11.GROUP.LEAVE` | 退出当前队伍 | legacy | not-run |
| `F11.GROUP.ALLOW` | 允许/禁止组队邀请 | legacy | not-run |
| `F11.GROUP.ACCEPT` | 接受当前组队邀请 | legacy | not-run |
| `F11.GROUP.DECLINE` | 拒绝当前组队邀请 | legacy | not-run |
| `F11.GUILD.OPEN` | 打开公会及请求完整信息 | legacy | not-run |
| `F11.GUILD.INVITE` | 邀请成员 | legacy | not-run |
| `F11.GUILD.INVITE_ACCEPT` | 接受公会邀请 | legacy | not-run |
| `F11.GUILD.INVITE_DECLINE` | 拒绝公会邀请 | legacy | not-run |
| `F11.GUILD.NOTICE` | 编辑发布公会公告 | common limitation | not-run |
| `F11.GUILD.KICK` | 踢出成员 | common limitation | not-run |
| `F11.GUILD.MEMBER_RANK` | 修改成员阶级 | common limitation | not-run |
| `F11.GUILD.RANK_NAME` | 修改阶级名称 | common limitation | not-run |
| `F11.GUILD.CREATE_RANK` | 新增公会阶级 | common limitation | not-run |
| `F11.GUILD.RANK_PERMISSIONS` | 保存八项阶级权限（含仓库存取） | common limitation | not-run |
| `F11.GUILD.MEMBERS_OFFLINE` | 显示/筛选离线公会成员 | legacy | not-run |
| `F11.GUILD.STORAGE_REFRESH` | 请求公会仓库完整112格 | legacy | not-run |
| `F11.GUILD.STORAGE_STORE` | 按当前UID/槽位/权限存入公会仓库 | legacy | not-run |
| `F11.GUILD.STORAGE_RETRIEVE` | 按当前UID/槽位/权限取出公会仓库物品 | legacy | not-run |
| `F11.GUILD.GOLD_STORE` | 向公会银行存金币 | legacy | not-run |
| `F11.GUILD.GOLD_RETRIEVE` | 从公会银行取金币（服务器验证权限） | legacy | not-run |
| `F11.GUILD.BUFF_LIST` | 查询公会Buff目录 | legacy | not-run |
| `F11.GUILD.BUFF_ACQUIRE` | 确认并取得指定公会Buff | legacy | not-run |
| `F11.GUILD.BUFF_ACTIVATE` | 按CanActivateBuff确认激活已有Buff | legacy | not-run |
| `F11.FRIEND.OPEN_REFRESH` | 打开/刷新好友列表 | legacy | not-run |
| `F11.FRIEND.ADD` | 具名添加好友 | legacy | not-run |
| `F11.FRIEND.REMOVE` | 按真实角色索引删除好友/解除黑名单 | legacy | not-run |
| `F11.FRIEND.MEMO` | 保存好友备注 | legacy | not-run |
| `F11.FRIEND.MAIL` | 给选中好友写信 | legacy | not-run |
| `F11.FRIEND.WHISPER` | 给选中好友私聊 | legacy | not-run |
| `F11.FRIEND.BLOCKED_TAB` | 切换好友/黑名单页 | legacy | not-run |
| `F11.BONDS.MARRIAGE_ALLOW` | 切换接受求婚 | legacy | not-run |
| `F11.BONDS.MARRIAGE_REQUEST` | 向面对玩家求婚 | legacy | not-run |
| `F11.BONDS.DIVORCE` | 请求离婚 | legacy | not-run |
| `F11.BONDS.MENTOR_ALLOW` | 切换接受师徒邀请 | legacy | not-run |
| `F11.BONDS.MENTOR_ADD` | 具名请求师徒关系 | legacy | not-run |
| `F11.BONDS.MENTOR_CANCEL` | 解除当前师徒关系 | legacy | not-run |
| `F11.BONDS.MARRIAGE_ACCEPT` | 接受当前求婚 | legacy | not-run |
| `F11.BONDS.MARRIAGE_DECLINE` | 拒绝当前求婚 | legacy | not-run |
| `F11.BONDS.DIVORCE_ACCEPT` | 接受当前离婚请求 | legacy | not-run |
| `F11.BONDS.DIVORCE_DECLINE` | 拒绝当前离婚请求 | legacy | not-run |
| `F11.BONDS.MENTOR_ACCEPT` | 接受当前师徒请求 | legacy | not-run |
| `F11.BONDS.MENTOR_DECLINE` | 拒绝当前师徒请求 | legacy | not-run |
| `F11.BONDS.SPOUSE_MAIL` | 配偶名非空时打开具名 Mail 本地草稿；此操作不发送邮件 | legacy | not-run |
| `F11.BONDS.SPOUSE_WHISPER` | 配偶名与收到的 mapName 非空时把聊天草稿填为 :)；不拼接收件人、不发包 | legacy | not-run |
| `F11.TRADE.REQUEST` | 向玩家请求交易 | legacy | not-run |
| `F11.TRADE.ACCEPT` | 接受当前交易邀请 | legacy | not-run |
| `F11.TRADE.DECLINE_CANCEL` | 拒绝邀请/取消交易 | legacy | not-run |
| `F11.TRADE.ITEM_STORE` | 从实际Bag槽位放入十格报价 | legacy | not-run |
| `F11.TRADE.ITEM_RETRIEVE` | 从真实报价槽位取回物品 | legacy | not-run |
| `F11.TRADE.GOLD` | 修改金币报价 | legacy | not-run |
| `F11.TRADE.LOCK` | 锁定报价 | legacy | not-run |
| `F11.TRADE.UNLOCK` | 解锁当前报价 | legacy | not-run |
| `F11.TRADE.COMPLETE` | 双方锁定后完成交易（真实服务器结果） | legacy | not-run |
| `F11.RANKING.OPEN` | 打开排名并请求当前查询 | legacy | not-run |
| `F11.RANKING.TYPE` | 选择总榜/职业榜 | legacy | not-run |
| `F11.RANKING.ONLINE` | 切换在线过滤并保持当前榜种 | legacy | not-run |
| `F11.RANKING.PREVIOUS` | 排行偏移上一行（-1；20行窗口；clamp0..count-20） | legacy | not-run |
| `F11.RANKING.NEXT` | 排行偏移下一行（+1；20行窗口；clamp0..count-20） | legacy | not-run |
| `F11.RANKING.INSPECT` | 查看排名真实玩家身份 | open | not-run |
| `F11.GROUP.KICK` | 移除选中组员 | legacy | not-run |
| `F11.FRIEND.BLOCK` | 具名添加黑名单 | legacy | not-run |
| `F11.HERO.SKILLS_OPEN` | Ctrl+S 打开Hero Skills页 | legacy | not-run |
| `F11.HERO.BAG_CELLS` | 显示Hero物理Bag 2..41（按容量） | legacy | not-run |
| `F11.HERO.EQUIPMENT` | 显示14个Hero装备槽 | legacy | not-run |
| `F11.HERO.STATUS` | 显示Hero Status属性页 | shared | not-run |
| `F11.HERO.STATE` | 显示Hero State统计及经验页 | shared | not-run |
| `F11.HERO.MOVE` | 在Hero物理Bag/两格belt间移动 | legacy | not-run |
| `F11.HERO.EQUIP` | Hero装备Bag物品 | legacy | not-run |
| `F11.HERO.REMOVE` | Hero卸下装备到实际Bag | legacy | not-run |
| `F11.HERO.MERGE` | 合并Hero实际兼容堆叠 | legacy | not-run |
| `F11.HERO.TRANSFER` | 从玩家Bag转交Hero | legacy | not-run |
| `F11.HERO.TAKE_BACK` | 从Hero取回到玩家Bag | legacy | not-run |
| `F11.HERO.USE` | 使用Hero当前Bag/belt物品 | legacy | not-run |
| `F11.HERO.USE_CONFIRM` | shape4消耗品显式确认后使用 | legacy | not-run |
| `F11.HERO.AUTO_HP` | 设置Hero自动HP药阈值 | legacy | not-run |
| `F11.HERO.AUTO_MP` | 设置Hero自动MP药阈值 | legacy | not-run |
| `F11.HERO.AUTO_HP_ITEM` | 选择Hero自动HP药真实itemIndex | legacy | not-run |
| `F11.HERO.AUTO_MP_ITEM` | 选择Hero自动MP药真实itemIndex | legacy | not-run |
| `F11.HERO.MAGIC_KEY` | Hero技能赋键17..24或清除0 | legacy | not-run |
| `F11.HERO.BELT_SHOW` | Hero默认独立两格belt显示 | legacy | not-run |
| `F11.HERO.DRAG` | 独立Hero窗口拖动保持位置及pointer/epoch lease | legacy | not-run |
| `F11.PET.OPEN` | 打开宠物窗口请求更新 | legacy | not-run |
| `F11.PET.CLOSE` | 关闭宠物窗口停止更新请求 | legacy | not-run |
| `F11.PET.SUMMON` | 召唤选中宠物 | legacy | not-run |
| `F11.PET.DISMISS` | 召回当前宠物 | legacy | not-run |
| `F11.PET.RELEASE` | 按当前名字确认释放宠物 | legacy | not-run |
| `F11.HERO.INVENTORY_OPEN` | Ctrl+I 独立开关Hero Inventory | legacy | not-run |
| `F11.HERO.CHARACTER_OPEN` | Ctrl+C 独立开关Hero Character | legacy | not-run |
| `F11.HERO.BELT_RESTOCK` | 已接受belt用药后唯一安全候选补货 | legacy | not-run |
| `F11.HERO.BELT_CLOSE` | 关闭独立两格Hero belt | legacy | not-run |
| `F11.HERO.BELT_ROTATE` | 两格Hero belt横竖旋转 | legacy | not-run |
| `F11.PET.SELECT` | 选中实际宠物槽 | legacy | not-run |
| `F11.PET.RENAME` | 一次许可确认宠物更名 | legacy | not-run |
| `F11.PET.MODE` | 切换宠物拾取模式 | legacy | not-run |
| `F11.PET.OPTIONS` | 打开宠物选项 | legacy | not-run |
| `F11.PET.FILTER` | 保存九项拾取过滤 | legacy | not-run |
| `F11.PET.GRADE` | 保存拾取等级过滤 | legacy | not-run |
