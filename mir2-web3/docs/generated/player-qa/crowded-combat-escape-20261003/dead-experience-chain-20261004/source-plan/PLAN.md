# 死亡经验、升级和复活权威链：只读计划 01

日期：2026-10-04。工作区：
`C:/Users/Administrator/.codex/worktrees/crowded-combat-control/mir2-player-journey/mir2-web3`。
游戏基线 R17 为 `6032ef8b3e27dd97bad0b20c8676ef9f185db64b`；本轮读取的工作
基线从 `3e1f9167097ff239faeffdde809a17b2c0009b6c` 出发，包含刚完成但冻结待整合的
同图 EXPOwner 修改。root 正在整合地图，逐文件来源、实际 HEAD 和读取时间以
`source-index.json` 为准；不能把本计划当作已提交/已发布的版本。

本轮只有源代码审计和拟议测试，没有编辑仓库代码、测试、全局文档或旧证据；
没有 Cargo、发布、服务器操作或读取真人存档。下面所有 RED 都是**待执行**，
上一轮 Zone outbound 通过不等于本轮个人/Gateway/存档链已验收。

## 结论

1. 当前真实风险是用 HP 正负推导死亡和复活。死亡毒主人可以合法收到经验；
   一旦越过升级阈值，个人 runtime 补满 HP，Gateway 在看到 LevelChanged 后
   将 HP 同步给 Zone；Zone 随即清掉 Dead、推进 life_generation 并清理状态/动作。
   没有正常 Revived 生命周期也能恢复活人资格。
2. 原版在线 `Dead` 与 HP 独立：死亡后升级可以 HP/MP 满值但仍然 Dead，不能行动，
   正常 TownRevive 仍有效。“升级时保持 HP0”是实用安全修复，**不是完整 1:1**。
3. 原版角色存档不单独保存 Dead。正常新登录是新 PlayerObject；HP0 的存档被
   补满并转移到绑定安全区/PK town，HP>0 则不走这个分支。因此死亡后升级、
   再真实退出重登会以新对象活着出现于原合法位置。这是源代码观察，不应误改成
   “把永久 Dead 字段写进角色存档后跨登录恢复”。
4. 同一在线 epoch 的事务回滚/重放不能使用上面“新登录”的规则；必须保留独立
   死亡标记和原身份。正常掉线保留 runtime 的 authenticated resume 也要与真正
   logout/despawn 后重新 StartGame 区分，不能仅看持久化 HP。
5. fenced teardown 还有独立出口：先缓存 Zone vitals，再 checked 提交奖励，最后
   用缓存 HP/MP 覆盖个人数据；checked 分支没有普通分支的升级 vitals 同步。
   退出时消费 queued poison award 必须另测，不能复用正常 tick 的通过结论。

## 原版规则，包含保存与加载

来源均为 `E:/mir2/Crystal` 本地原始源码，冻结节选与完整文件哈希在本目录。

| 来源 | 已核对的行为 |
| --- | --- |
| `Server/MirObjects/MapObject.cs:69` | 物体有独立 bool Dead；新物体的 bool 缺省为 false。 |
| `Server/MirObjects/HumanObject.cs:219` | CanGainExp 是默认 true 的独立字段；不是 `!Dead` 属性。225 的 `!Dead && !Observer` 属于 Blocking，不能误读为 EXP gate。 |
| `Server/MirObjects/MonsterObject.cs:1495–1509,982–990` | 周期毒认领/延期 EXPOwner；死亡奖励只要求主人 Node 存在、无 Master、合法 race，不按主人 Dead 拒绝。 |
| `Server/MirObjects/PlayerObject.cs:794–848,891–938` | Solo WinExp 调用 GainExp；GainExp 没有 Dead gate。原组队分支排除死亡组员，不在此次改写范围。 |
| `Server/MirObjects/HumanObject.cs:845–865` | LevelUp RefreshStats、SetHP(max)、SetMP(max)。SetHP 仅在当前非 Dead 且 HP0 时触发 Die，没有 positive-HP 清 Dead。 |
| `Server/MirObjects/PlayerObject.cs:643–650` | 真死亡同时写 HP0、Dead=true 并发 Death/ObjectDied。 |
| `Server/MirObjects/HumanObject.cs:117–157,552` | 行动资格和再生看独立 Dead；死亡满血仍不能移动、攻击、施法、常规再生。 |
| `Server/MirObjects/PlayerObject.cs:1392–1443` | TownRevive 检查 Dead，明确置 false，补满，移动到 bind/PK town，发 MapChanged、Revived/ObjectRevived。 |
| `Server/MirObjects/HumanObject.cs:1690–1721` | 正常同地复活也明确清 Dead，再发 MapChanged/Revived/ObjectRevived；单纯 HealthChanged/LevelChanged 不等于复活。 |
| `Server/MirDatabase/CharacterInfo.cs:57,176–180,414–424` | 读写 HP、MP、EXP 和地图/绑定位置；整文件没有独立 Dead 字段。 |
| `Server/MirObjects/HumanObject.cs:59–68` | HP/MP 属性直接写 Info.HP/MP，所以死后升级后的满血确实进入角色保存值。 |
| `Server/MirObjects/PlayerObject.cs:206–264,1069–1107` | Load 新 PlayerObject 后 RefreshStats；仅 HP==0 时回 bind/PK town 并补满。合法 HP>0 保留位置，StartGame 再做地图有效性/NoReconnect 检查。 |

原版 HP0 普通重登与死后升级 HP>0 重登的差异必须分别测试。保留当前地图只指
合法普通地图；NoReconnect、无效位置、PK town 等原有分支不能一并放宽。

## 当前普通链与出口差异

| 路径/函数 | 当前行为和风险 |
| --- | --- |
| `zone/runtime/experience_ownership.rs::native_experience_reward_owner` 与 `native_monster_kill_outbounds` | 同图匹配 session/account/character/object 的死亡 solo owner 可发 award。只证明 Zone 选择，不能证明持久化到账。 |
| `zone/runtime.rs::tick_native_monster_damage_poison` / `resolve_native_monster_poison_damage` | 普通正值绿毒实际 tick 产生伤害/死亡/award；本轮使用这一公开接线，不依赖零值私有毒规则。 |
| `gateway/routing.rs:10094–10100` | 同一 drain 先消费玩家伤害、再消费怪物生命周期、再奖励。因此可以在正常排队顺序下先确认 owner 死亡，再产生 XP。 |
| `runtime/drops.rs:3535–3547,3604–3623` | 共享杀怪事务检查在场，推进任务、实际 GainExp；没有再次按 alive gate 丢掉 dead-solo 奖励。 |
| `runtime/shared_kill_experience.rs:28–177` | 验证活动身份、immutable key/payload、已持久化 kill journal；已提交相同 key 返回空 packets，冲突 payload 拒绝。现有 receipt 合同需保留。 |
| `runtime/prepared_kill_experience.rs:65–249` | 已知失败回滚、AlreadyCommitted 恢复 durable source，unknown outcome 冻结；不是“失败就补发”逻辑。 |
| `runtime/leveling.rs:210–224` | 补满 HP/MP。目前没有独立死亡状态，因此这一步令个人 current_player_is_dead 变 false。 |
| `runtime/components.rs:300–303` | current_player_is_dead 仅以 SelfPlayer vitals.hp<=0 判定；药品/NPC/移动/施法等多处依赖它。 |
| `runtime/combat.rs:383–455` | 重复死亡保护也仅用 was_alive = hp>0。满血尸体会被误认为新的可受击生命，可能重复进入死亡/掉落前奏。 |
| `gateway/routing.rs:10945–10952,9290–9312` | 见 LevelChanged 即把个人 HP/maxHP/MP 送 SyncPlayerVitals，没有独立 Dead 或明确 mutation cause。 |
| `zone/runtime.rs:1394–1407` | positive HP 对旧 Dead 被当作复活；清 poison、life_generation+1、清生命相关 actions。 |
| `runtime/session.rs:126–133,961–1023` | 内部 vitals snapshot 和强制 reconciliation 没有 Dead，无法表示“满血但仍死”。 |
| `runtime/packets.rs:7925–7932` | SelfPlayer 的 WorldEntitySnapshot.dead 同样从 HP 导出；即使只修 Zone，还会留下错误的个人/前端快照。 |
| `runtime/buffs.rs:447–453` | 正常药品恢复已有死亡 guard；独立 Dead 到位后应继续阻止满血尸体消费 queued potion。不要重写药品曲线。 |
| `gateway/routing.rs:9240–9283,14220–14226` | 现有 acknowledged-private-death 与 TownRevive 保护依赖 private HP0；死亡满血时保护失效，需要使用明确生命周期而不是 hp==0。 |
| `gateway/routing.rs:10384–10505` | 已结算伤害有object/generation/sequence去重和death penalty watermark；为重放先强制hp_before，末尾取Zone vitals且HP>0就移除owner_dead_entity_ids。必须保留这条可信回执链，但满血尸体不能据此被当作复活。 |
| `runtime/movement.rs:342–401` | TownRevive 检查 current_player_is_dead 后明确补满并发 Revived；独立 Dead 修好后该路径必须仍可复活满血尸体。 |
| `runtime/items.rs:2710–2734,3018–3024` 与 `zone/runtime.rs:7499–7518` | 既有复活卷/共享 Reincarnation 是明确复活路径；不能因为“preserve dead”将它们一并禁止。Reincarnation 不是新增经典1.76范围，只要求已有路径不回归。 |
| `runtime/save.rs:252–266,2715–2721,525–564` | 持久化只存 vitals/EXP；same-session restore 重建个人 ECS，必须保留在线 death。当前 StartGame 直接恢复 HP0，没有源码 Load 的 HP0→bind/full 分支，是已有独立 source gap。 |
| `world_runtime.rs:46,1021–1022` 与 `runtime/save.rs:3867–3895` | 已有 ReplayRetainedStartGameBootstrap 独立于普通新登录；只是保留角色的内部 bootstrap。需要给它单独 life 回归，不能统一套新登录 reset。 |
| `runtime/shared_guild_experience.rs:162–171,286–295,349–361` | XP rollback checkpoint 有 save/buff/NPC/queue/tick/hero，没有独立 player-life；新增 transient life 后须捕获和恢复。 |
| `gateway/routing.rs:10158–10224,10865–10910` | teardown 在10171缓存 Zone vitals；10191 checked 提交奖励且丢弃返回 packets；10216使用缓存 HP/MP覆盖个人再取最终 checkpoint。升级后的 maxHP/level 与 HP可能分离。不可简单跳过最后 authority reconciliation。 |
| `gateway/routing.rs:26602–26662` | 已有活人升级测试直接构造 award 调用内部消费者；没有普通毒→死亡→升级、保存重登或 dead-action 拒绝证据。 |

另外 `filter_stale_owner_vital_packets`（routing.rs:10721–10745）按 HP 判断 Death/
Revived 的旧 packet 去留，也要把“死亡满血”作为回归条件；不能误把原死亡展示
包当作 stale revive 后的旧包清掉。life_generation 仍是死亡/复活屏障，不能代替
account/session/object 的登录身份。

## 候选修复，推荐独立在线死亡状态

先跑下面的完整 RED，再由 root 授权一名 code writer。推荐让个人 PlayerRuntimeResource
拥有明确的 online player-life 状态；Zone 已有 `ZonePlayer.dead`。所有同步都保持
两者一致：普通 level-up/stat/vitals mutation 保留死亡状态，只有可信 Death、
明确 Revived/TownRevive/复活卷/既有 Reincarnation 或新 PlayerObject 登录改变它。
不要把 bool 当作普通 ClientPacket 的可写字段，不新增生产 QA/admin API。

保留 LevelUp 的满 HP/MP 原行为。Gateway 从活跃 Zone 身份读取 life，与个人
镜像同步；正常奖励提交后的新 pools/HP/MP 回写 Zone 时明确表示“保持 life”，
不能重新用 HP 推断复活。Zone 的 revive generation/actions 清理只能在实际
复活 transition 发生一次。普通 TownRevive 要基于 Dead，即使 HP 已满也合法。
共享游戏的 life 仍由单写 Zone 权威裁决；个人字段是可信镜像和现有兼容路径的
状态，不创建第二份共享世界。旧 private acknowledged death 的既有保护要保留，
但不能让任意正值 personal HP 绕过明确复活许可。

durable CharacterSaveRecord 继续没有 Dead 字段；同在线事务 checkpoint 必须
有 transient player-life，restore 已认证相同身份的 source 时保留它。真正新
StartGame 按源 Load 规则创建 alive 对象，HP0时走 bind/full；HP>0沿合法保存
位置继续。authenticated retained-session resume 需要保留原 live state，不借
StartGame bootstrap reset 把尸体变活人。

teardown 不能靠丢掉 final Zone vitals 解决。需用已确认的 award receipt/活动身份
在同一个 fenced 顺序中提交新 level pools，同时保持死亡状态；最后保存仍以
该 fence 的最新权威 state 为准。重复 award/失败不得额外升级、改变 dead 或发
假 LevelChanged/Revived。unknown publication 沿已有 freeze/recovery 协议处理。

这是跨个人/共享/持久化消费边界的修复，忠实候选不应宣称两行改动能闭环。
下表为最窄行为候选；实际 hunk 和 compiler companions 需 root 再授权，尚未编辑。

| 候选文件 | 最窄必要范围 |
| --- | --- |
| `runtime/resources.rs`、`runtime/components.rs` | 独立 personal life，中央 current_player_is_dead；不修改 Hero/Monster 的死亡数据。 |
| `runtime/combat.rs` | 首次真正死亡标记、重复/late damage 不再用 positive HP 误判新生命。 |
| `runtime/session.rs`、`world_runtime.rs` | typed vitals/life snapshot 与内部权威镜像；显式方法封闭在 trusted wrapper。 |
| `runtime/packets.rs` | SelfPlayer.dead 视图用 life；不全局重写协议。 |
| `runtime/movement.rs`、`runtime/items.rs` | TownRevive/既有复活卷明确清 life；保留原身份、地图和物品消费。 |
| `runtime/save.rs` | PreserveSameSession 保持 life，ResetForNewSession 按原 Load HP0处理；不加入永久 Dead schema。 |
| `runtime/shared_guild_experience.rs`、`runtime/prepared_kill_experience.rs` | transient player-life capture/restore；保留现有 source CAS/journal/rejection/unknown 合同。 |
| `zone/runtime.rs`、`zone/types.rs` | trusted life-aware vitals mutation，普通 positive HP不复活；既有 ZonePlayer.dead/checkpoint字段继续覆盖。 |
| `gateway/routing.rs` | 四个边界：普通 award、两向 life reconciliation、明确 revive、fenced teardown/replay；packet stale filtering引用明确 life。 |
| 若需要 `zone/manager.rs` | **仅 typed getter/命令转发编译 companion，需另获批准**；不做 manager 身份、跨图 EXP/logout 仲裁、组队或掉落改写。 |
| 新 `apps/simulation/tests/dead_experience_chain.rs` | 公开个人/Zone服务端入口；不要塞 tests.rs。 |
| 新 `apps/gateway/src/routing/tests/dead_experience_chain_tests.rs` 与1行注册 | 真实普通客户端包 + 实际 Zone outbounds + source receipt/save/relogin链；可复用现有私有 trusted测试 scaffolds。 |

`runtime/leveling.rs` 现有 refill 本身符合源规则，不应默认改掉它。若 writer 选择
HP0实用方案，最小行为变化可在其 refill处加死亡 guard，但必须将 source parity
标为未闭合，且单独记录 HP保存/重登差异；不能通过弱化 below full-HP断言将其
命名为原版 Candidate。此计划推荐忠实状态方案而非这条 shortcut。

## 必须先跑的真实 RED 计划

共用 fixture：新隔离 account/character，真实 Login/StartGame，Taoist 合法学习
Poisoning并持有真实 powder。装备/技能/初始EXP/monster HP/DC 可由可信测试配置
确定，严禁改真人 save、生产 QA packet 或直接构造 kill award 冒充完整链。
使用小型 server-authored map 配置、实际 SpawnMonster/native AI和普通 Magic。
目标至少一次普通绿毒命中；另一真实怪物通过常规 tick 给 owner 致命伤害。
确认 owner Death/ObjectDied 后，目标在下一个正值 poison tick 死亡。使用实际
Zone award、Gateway selection/durable key/queue drain，核对 actual GainExperience、
quest target identity和完整 durable source。固定 EXPrate、无组队/Guild/social bonus，
以源经验阈值构造“差一次击杀升级”；留其他账号/observer证明 recipient 不混淆。

| 拟议用例 | 核心断言 | 当前代码预计结果（不是已执行） |
| --- | --- | --- |
| `dead_solo_poison_award_without_level_remains_dead_and_durable` | XP/任务和receipt精确一次；个人与Zone同一account/character/object，Dead=true/HP0、无Revived；save/reload不重复领。 | 非升级基础应可通过，作为对照，不能只添加预计失败用例。 |
| `dead_solo_poison_level_up_refills_pools_but_preserves_online_death` | 实际 poison击杀跨阈值；个人+Zone level、HP=maxHP、MP=maxMP一致，同时Dead=true；life_generation不因升级推进、无Revived/ObjectRevived。 | 应在Dead/生命屏障断言上失败。 |
| `dead_full_hp_rejects_walk_run_melee_cast_and_normal_items` | 对升级尸体发送真实Walk/Run/Attack/Magic/UseItem；位置/MP/物品不变、无有效damage/动作broadcast；NPC动作也拒绝。 | HP推导死状态将失败；当前许多保护会把它当活人。 |
| `town_revive_accepts_dead_full_hp_and_is_exactly_one_life_transition` | 上例随后普通TownRevive；绑定地图/坐标正确、Revived和observer ObjectRevived仅一次、Dead=false，后续普通移动/施法恢复；重复TownRevive no-op。 | 当前误alive会使TownRevive no-op。 |
| `late_damage_or_stale_packets_after_dead_level_does_not_repeat_death_penalty` | 相同已结算/旧generation delta不重伤、不新增第二次掉落/租赁返还或Death；HP/full Dead维持；正常明确复活后新的合法生命仍可再次死亡。 | 当前HP-derived was_alive和generation推进有风险；具体失败先真实测。 |
| `dead_poison_level_up_during_fenced_teardown_saves_latest_trusted_pools` | 将真实award留到LogOut/transport-disconnect teardown；实际durable EXP/level/maxHP/HP/MP匹配源规则和receipt；未知结果不放行假成功。 | checked分支遗漏sync+旧vitals覆盖预计失败。 |
| `real_logout_relogin_distinguishes_zero_hp_death_from_dead_level_full_hp` | HP0死亡真正logout/start：新对象alive、bind/full；死后升级满HP真正logout/start：新对象alive、合法原位置；两者EXP和receipt保留一次。 | HP0重登回bind分支当前缺失；fullHP已有表现不能掩盖在线dead问题。 |
| `authenticated_resume_preserves_dead_epoch_while_new_login_resets_it` | 保留runtime的正常认证resume仍Dead=true/fullHP，bootstrap不得把它变活；真实logout后freshruntime按上行新登录规则。 | 待测；禁止以death life_generation当spawn identity。 |
| `duplicate_and_conflicting_award_do_not_relevel_or_revive` | 复用真实immutable key与相同payload：不再加EXP、不增level、不发第二次LevelChanged/Revived；同key改payload拒绝；保存/reload仍一致。 | 既有journal护栏应保持，需加入life核对。 |
| `known_save_failure_rolls_back_xp_pools_death_and_retries_exactly_once` | 用既有isolated失败repository，在真实poison award发布处失败：无成功GainExp/LevelChanged，source及transient life回到before、queue保留原key；恢复后一次到账；old receipt replay仍一次。 | 新独立life容易在restore重建中丢失，必须验证。 |
| `unknown_publication_freezes_without_fake_xp_or_revive` | 既有OutcomeUnknown路径拒绝后续写，按已有durable recovery对账；不猜before/补发；活动身份一致。 | 既有合同对照，不能为了green改变freeze语义。 |
| `legitimate_revive_scroll_and_existing_reincarnation_keep_their_life_fences` | 本次life改动不吞合法revive；有真实物品/既有已学习法术时原消费和generation一次；没有资格的请求保持拒绝。 | 邻接保护，非新增expanded玩法验收。 |

测试结束后再做孤立 Gateway 的真实 TCP/WSS 两账号 ordinary packet 验证；单机
private calls、公用 fixture或constructed award都不能替代该证据。此阶段普通包
不需要高风险生产 debug入口。原生客户端截图/实际feel仍是独立Frontend Gate。

## 验证顺序和停止边界

1. root 完成当前地图/归属整合后固定实际 commit/逐文件hash，注册独立 tests。
   首先执行新 complete-chain RED，完整保留 stdout/stderr、失败原因和实际普通包。
2. root授权最窄 hunk 后修 life与普通奖励，重跑上述focused GREEN；再补source
   Load/transaction/checked-teardown边界。不能靠大量旧suite掩盖缺少真正RED。
3. 必要邻接只覆盖：既有live-level-up、TownRevive、dead potion、source failclosed/
   prepared kill、共享生命generation/reincarnation、checkpoint/currentRoot、新21项
   ordinaryownership。Rust1.95.0、jobs<=2、复用gateway-tests-r49，不清缓存。
4. 原版“满血但dead”原生投影和新登录源码规则若未实现，应精确保留为未验收；
   不宣布全链100%、新包或发布完成。root负责全局ledgers和rollout。

禁止扩展manager的跨图身份/归属、地面60秒、组队公式、generic Boss策略或日常/
周常经验。此 worker 交付只读计划后停下，任何实际源代码修改须新授权。
