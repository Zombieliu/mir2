# Web / Windows 有界行为矩阵08

## Source22：Bag→Belt源码候选闭合（2026-10-07）

仅`F05.belt.move-from-bag`由open转为legacy/sourceCandidate closed：实际CompatDOM Bag与默认sharedBevy Bag均可拖到六个真实Belt button，覆盖空槽及occupied swap、UID0和Bag2。共享Core纯planner返回raw from=globalBagSlot+6、to=0..5，原MoveItem仅保留type/grid/from/to四键，grid=belt；不发送EquipSlotItem。旧native outbound误注通过追加来源更正说明，originalAudit/native历史字段保留。共享规则和移动端复用不将整个DOM Belt painter升为shared类别，也不强制compat模式。

原始inventoryCapacity含六Belt槽，canonical值仅46/54/58/62/66/70/74/78/82/86；Native无效容量fallback46，严格Web ABI拒绝非canonical。源实例完整raw来源与全局唯一UID必须存在，UID0保留，Bag2首格global40映射raw46。缺可选PUI能力只拒绝新动作，不破坏普通Core接口；JS不另做索引规划。

真实Page/Shell/Belt源码的有限DOM/ABI夹具检查覆盖最后Core getter及pending重入、旧指针失效后恢复、second pointer隔离、四logical px拖动阈值、六真实槽geometry/elementFromPoint/隐层/scale/DPR、blur及successor cleanup。terminal先burn再callback，最后owner/source/layout、parity/mail/social/storage门槛及opaque proof紧邻dispatcher.enter与真实socket.send。已进入或unknown发送保护source/target UID及两格，不因超时/owner/等价layout清锁、不自动重试；精确grid/from/to/success同owner ACK失败可释放，成功还需更新完整source/target raw snapshot，ACK与snapshot两序都验证后才释放。这里的“真实”指受测产品源码入口，未操作实际页面或连接。

Root八组当前Rust实际230次跨配置通过、3项原ignored，21项新增distinct（Core3/Native1/PUI5/portable Bag9/Runtime Bag3）全部执行。最终Node03原11脚本218/218、0失败/0跳过；Stage5原299＋新增19＝318内部组只计一个文件级Node测试，不能218＋318。严格code-only非增量TSC03通过。Storage原65具名组标签/断言行保留；Combat原23组标签/顺序保留，三处必要AST位置期望+3→+4及末尾2→3保留原Repair/Auth并追加BagProof，装配真实Shell依赖，不声称全部旧assert字节不变。

当前Core/PUI实际build02退出0：Core WASM252417 B/JS24393 B、版本cdbc1b7c…字节和版本不变，source指纹bf004f2a…；新PUI WASM221108 B/JS17931 B、版本c5517ce7…，满足原WASM<262144 B及JS≤204800 B预算。三renderer实际build01退出0、版本bevy-b8b16a3145ac75f2并通过原预算。Native实际fresh build01退出0、757642ms，EXE104760832 B/SHA256`400f36509204e4983a3f620641333d721b3042c81656bd37035c48d182e0a1f2`，未启动。Renderer immutable release沿既有.gitignore列为生成资源，文档不证明发布。

Next01实际退出0、159683ms，dist`.next-web-windows-catchup-20261007-05`，strict TypeScript24.1s/13静态页；27417声明输入、19007文件/308目录含根/604720042 B。61 NFT/37094引用（37092 regular＋2 declared junction），0missing/private，junction未遍历。Thin02实际退出0、40097ms，包`.mir2-thin-client-web-windows-catchup-20261007-05`，63166声明输入、7301文件/776目录含根/0links/372988592 B；原377487360 B cap余4498768 B。独立pureFS复核完整输入/输出/包树、11 runtime copies，0确认P0/P1/P2、0执行/0写；44 warning完整多重集不变，231 JSON source/output pairs实际hash与Source21一致，仅限定承接历史token结论，未新跑parser/lexer。旧Next04输出成员被替换，但一项04/types/routes.d.ts作为既有Node03/strict TS输入重新纳入，未进入当前NFT/包；不能声称完全没有04路径。

15次实际CargoGuard调用严格串行，原threshold53687091200 B与fresh≤2000ms策略保持，每次actual GetVolume C≥50GiB，sampleEnd→childStart实际61–97ms，PolicyB退出/Dispose闭环。Node01实际151/218、67失败历史保留，真实依赖夹具与过期Core修正后02及最终03通过；TSC01仅prepared未跑。Corebuild01因receiptRoot不存在被原guard90挡在Cargo前，Core02仅先建收据目录后通过；Thin01因不可变process helper拒绝反斜杠参数而prelaunch失败，Thin02仅将data参数改为正斜杠后通过，原日志/配置/回执保留。

当前103 shared/203 legacy/3 open/8 common limitation；仅一行闭合，其余316整row、317 ordered IDs、全部originalAudit/native与historical rawCounts保持，player全部not-run。最终Matrix08实际row有限字段展平273关联/140唯一声明、每row去重272；仅新增19项Stage5声明关联，保留F01.safe-key.open原Overlay行内重复1次。关联数不等于Rust21、新执行数、Node218、Stage5318或玩家验收。317为有界审计记录，没有完整验收分母或overall percentage，Candidate100=false、goal active；源码/CPU有限检查/静态构建不证明可玩。

Source21已实际commit/push并经HTTPS remote核验`229ae772d7bd3acdb1b21ef3e8005bf817a393d4`。Source22 commit/push在证据创建时pending，由Root实际完成后报告。下一项Source23处理`F02.world.fishing-click`，需实际共同local pose/action与raw transform类型事实，不猜standing；随后`F09.NPC.PEARL`、`F11.RANKING.INSPECT`。这三条raw open不是整体剩余分母。UI/HTTP/socket/WASMAPI或实例、登录/游戏/战斗/保存重登、移动真机及最终frontend验收均未运行，用户“继续代码，暂不操作界面”持续有效。

固定Windows基线`f72e36fb84c3574fff0aeb2abed856454b14289d`，Source22父提交为已发布Source21 `229ae772d7bd3acdb1b21ef3e8005bf817a393d4`。[矩阵07](feature-matrix07.md)与全部更旧矩阵保留历史。当前证据均为Root实际回执与独立只读复核；文档作者不执行测试、构建或玩家操作。

| 当前证据 | 字节 / SHA256 |
| --- | --- |
| [bag-belt-source22-action-chain-review01.json](bag-belt-source22-action-chain-review01.json) | 38726 / `93d43c966a822bedb6ff3365b46d0d48be7da03e16f25eaa20dd8405f2289695` |
| [bag-belt-source22-combined-build-result01.json](bag-belt-source22-combined-build-result01.json) | 86529 / `8e4649437b842443fad614b610bad11764cf7fbcaf5a7e6ecab26f8a9d21b6b6` |
| [bag-belt-source22-finite-result01.json](bag-belt-source22-finite-result01.json) | 75003 / `a51fdd3412a2365cd12835fc2295ab2ffb75f0f6aef5dcdc3154c051a25dade2` |
| [feature-matrix08.json](feature-matrix08.json) | 3019610 / `9e5577449d9b0a495dd5358d3a60b3b057b15c414e5921294d5d35c565132236` |

[Current Evidence08](current-evidence08.json)由Root在本轮MD写入后生成，文档不预设其hash。

## 一条当前Bag→Belt行

| ID | 当前源码状态 | 玩家状态 |
| --- | --- | --- |
| `F05.belt.move-from-bag` | legacy/sourceCandidate closed；CompatDOM和默认sharedBevy Bag真实来源→六Belt targets→共享Core plan→四键MoveItem→精确ACK与新完整snapshot | not-run |

原native.outbound的EquipSlotItem标签是历史误注，完整originalAudit/native保持不变；Source22追加当前来源更正：实际MoveItem grid=belt、from=globalBagSlot+6、to=0..5。详见JSON source22Update.nativeOutboundCorrection、行内evidenceCorrections及动作链复核。本矩阵不从历史误注推导真实装备动作。

## 具名有限检查关联

按row有限字段declarations或兼容declaration数组展平，identity使用path＋name/declaration；实际Matrix07为254关联/121声明，Matrix08为273关联/140声明，每row去重272。Source22只新增19项Stage5声明关联，F01.safe-key.open保留1处Overlay行内重复。21项新增Rust distinct及230次跨配置执行独立记录于有限结果，不加到这些关联数。DOM/ABI为显式有限夹具，不能等同浏览器、WASM实例、网络或玩家验收。

19组共同覆盖本行动作链；每组只证明下表对应产品模块或提取函数在显式内存/DOM/ABI夹具内的有限范围，不能解读为每组独立执行完整Page→Shell→Belt端到端链。未运行浏览器、WASM、服务器或真实玩家。

| Stage5声明（实际源行） | 关联稳定ID | 本组实际覆盖范围 |
| --- | --- | --- |
| Bag-to-Belt optional PUI forwards UID zero and Bag2 capacity without JS index planning（5351） | `F05.belt.move-from-bag` | Actual optional PUI facade forwards UID0 and Bag2 capacity to an ABI-shaped mock; no WASM, Page or Shell execution. |
| Bag-to-Belt optional PUI rejects malformed results and reentrant ABI getter replacement（5369） | `F05.belt.move-from-bag` | Actual optional PUI facade rejects malformed ABI-shaped responses and reentrant getter replacement in memory; no WASM execution. |
| Bag-to-Belt dispatcher protects UID zero both identities and empty destination cells（5395） | `F05.belt.move-from-bag` | Actual dispatcher memory calls reserve both UID0 identities and source/target cells, including empty targets. |
| Bag-to-Belt dispatcher entered transport throw stays blocked without owner layout or timeout release（5408） | `F05.belt.move-from-bag` | Actual dispatcher memory calls retain entered unknown outcomes across owner/layout/timeout changes and reject replay. |
| Bag-to-Belt exact ACK ignores wrong tuple session and physical connection（5419） | `F05.belt.move-from-bag` | Actual dispatcher memory ACK calls reject wrong grid/from/to/session/physical connection tuples. |
| Bag-to-Belt success requires exact ACK and newer complete empty or occupied exchange in either order（5428） | `F05.belt.move-from-bag` | Actual dispatcher memory ACK/layout observations require exact newer complete empty or occupied exchange in either order. |
| Mail actual Belt decoder resolves from six and global Bag2 forty and protects both touched cells（5441） | `F05.belt.move-from-bag` | Actual mail decoder memory calls resolve raw Belt/globalBag2 indices and protect both touched cells. |
| Bag-to-Belt geometry requires six actual painted unobscured targets and exact release point（5471） | `F05.belt.move-from-bag` | Actual geometry helper memory calls check six painted targets, elementFromPoint and exact release point under explicit DOM mocks. |
| Bag-to-Belt geometry rejects DPR hidden node identity and measured rect changes（5487） | `F05.belt.move-from-bag` | Actual geometry helper memory calls reject changed DPR, hidden/disabled/stale node identities and rectangles. |
| Belt actual layout effect retains equivalent bindings and registers all six empty or occupied buttons（5499） | `F05.belt.move-from-bag` | Actual Belt component AST/effect fixture retains equivalent semantic bindings and registers six empty/occupied buttons. |
| Bag-to-Belt actual Shell terminal burns before callback keeps four pixel threshold and fences compatibility clicks（5529） | `F05.belt.move-from-bag` | Actual Shell extracted gesture functions in memory burn terminal before callback, enforce4 logicalpx and fence compatibility clicks. |
| Bag-to-Belt actual Shell second pointer quarantines through terminal without successor cleanup erasure（5570） | `F05.belt.move-from-bag` | Actual Shell extracted gesture functions in memory quarantine second pointers and preserve successor cleanup ownership. |
| Bag-to-Belt Page actual UID zero Bag2 gesture sends exactly four keys once and retains transport throw barrier（5643） | `F05.belt.move-from-bag` | Actual Page extracted final send slice in memory sends exact4keys once for raw UID0/Bag2 and retains entered transport-throw barrier. |
| Bag-to-Belt Page final entry rechecks reentrant owner source geometry mail and actual mutation gates（5655） | `F05.belt.move-from-bag` | Actual Page extracted final send slice in memory rejects reentrant owner/source/geometry/mail/mutation changes. |
| Bag-to-Belt Page final Core getter reentry cannot add mail social or storage ownership after last gate（5677） | `F05.belt.move-from-bag` | Actual Page extracted final send slice in memory performs final Core getter before actual mail/social/storage lock checks. |
| Bag-to-Belt Page actual complete snapshot observation and exact ACK release occupied exchange in both orders（5691） | `F05.belt.move-from-bag` | Actual Page snapshot/ACK functions in memory release occupied exchange only after exact ACK and newer full layout in either order. |
| Mail actual item mutation projection preserves known UID zero while parcel attachment sentinel remains null（5708） | `F05.belt.move-from-bag` | Actual item mutation projection memory calls preserve UID0 while original parcel attachment sentinel remainsnull. |
| Bag-to-Belt Page uses actual zero preserving projection under unrelated mail locks for empty and occupied Belt（5731） | `F05.belt.move-from-bag` | Actual Page extracted send slice in memory uses UID0-preserving raw projection under unrelated mail locks for empty/occupied targets. |
| Bag-to-Belt Page rejects duplicate global UID zero and missing raw source despite display identity（5743） | `F05.belt.move-from-bag` | Actual Page source resolver/send slice in memory rejects duplicate global UID0 and absent raw source despite displayed identity. |

## 当前记录集剩余代码差距

- `F02.world.fishing-click`
- `F09.NPC.PEARL`
- `F11.RANKING.INSPECT`

Source23需要实际共同local pose/action和raw transform类型事实；探索结果不构成产品更改或闭合。W6实际界面、玩家、移动真机与最终frontend验收继续依用户要求暂缓。

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
| `F05.belt.move-from-bag` | 背包drag落真实腰带槽含空槽 | legacy | not-run |
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
