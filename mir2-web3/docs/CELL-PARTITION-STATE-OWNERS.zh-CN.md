# Cell 分区状态所有权合同

2026-10-24。适用当前一逻辑地图一个权威写入者，以及后续固定 Cell 的单提交器模式。
本清单分类现有字段，不证明这些字段已能跨 worker 或主机迁移。字段引用按当前源码名登记；
新增字段必须先补本清单，再允许独立 Cell writer。

## 身份、版本和权限

`LogicalWorldId` 的 `legacy-execution-domain:` 命名空间把现有持久 `ZoneId`
解释为逻辑世界/线路名；它不是主机、worker 或地址。多个热图线路仍可使用相同的旧
`ZoneKey`。`WorldPartitionKey` 将 world 与旧 `PartitionKey` 包装，保留旧掉落键字节。
同一世界的物理迁移不得改 world。换线路属于另一个逻辑世界，不是 Cell crossing。

`StableActionId` 使用服务器准入流的正 `source_sequence`；重放、转发和迁移必须复用，
耗尽必须拒绝，不能从零重新开始。host、PartitionId、TopologyRevision、placement epoch
不进入稳定行为键。`EffectEnvelope` 的效果序号从零开始，和行为 ID 共同标识同次动作中的
一个效果。改变内容、目标生命或到期时间不能冒用同一效果收据。

玩家 online namespace/epoch 表示当前上线实体，`life_generation` 表示该实体的战斗生命；
第一条生命可以为零。怪物 `incarnation` 必须为正，不能只凭可复用的 `object_id` 命中复活体。
`EntityLifeRef`、`WorldMapEffectOwner`、envelope 和 replay receipt 都是可持久化描述，
不是当前在线证明、owner lease 或“已提交”证明。真正结算必须在 owner 提交边界查询当前生命，
重新验证既有租约，并把去重收据与实际修改放在同一可恢复事务。P3 才实现独立效果 ledger 和
跨分区原子应用；不能把本合同里的 receipt 比较函数当作该能力。
生命引用匹配也不替代原技能的存活、距离、目标关系、资源和时间窗口判定。

新增内部 action/entity/effect/effect-owner 合同版本为 1，未知版本和未知字段拒绝。
manager 准入合同将 checkpoint 显式升级为 v4 与独立 v4 摘要域；v2/v3 按原字段形状读取，
v3 先验证原 v3 摘要，拒绝注入新的 world/sequence/receipt 字段。旧快照恢复后由可信 factory
首次绑定世界。gateway factory 显式升为 v3，保存唯一 world key 的全局目录，旧 v2 走显式
迁移并按原逻辑执行域绑定；shared state 仍由原 schema 验证，目录不导入 owner lease。
当前并行随机流改造将 runtime checkpoint/canonical 显式升级到 v8：v1–v7
先按原摘要域严格验证，再生成一次新的安全随机流；v8 保存 seed/counter 并校验摘要。
不把新字段偷偷加入旧 canonical 摘要域。该升级的最终测试证据由主题验收记录登记。

## 分类规则

| 标记 | 唯一 owner 与恢复策略 |
| --- | --- |
| C | 业务 canonical：原 ZoneRuntime/地图单提交器持有；完整保留内容、绝对时间、序号、随机结果和顺序，不能复制成第二个可写 owner |
| E | canonical 效果：首版仍由原地图 effect owner 唯一持有；未来实体切片只携引用/水位，不复制 canonical 待到期任务 |
| L | 在线活状态/能力：只能经同进程可信转移或独立上线交接协议保留；冷 checkpoint 不足以恢复权限 |
| D | 派生：从 canonical 重建并校验；只读 Ghost、索引或 ECS 镜像不能反向授权写入 |
| K | 配置/内容：由已校验版本的服务器配置/地图内容重建；迁移验证相同 build/data/config hash |
| G | gateway/账户事务：个人 Session、持久账户或现有共享经济提交边界持有；Cell 不复制或重新发放 |

## ZoneManager：全部直接字段

| 字段 | 分类 | owner、快照和固定 Cell 合同 |
| --- | --- | --- |
| `zones` | C | 每个逻辑 Zone 保留原完整 runtime；固定 Cell 单提交器不能复制一份 runtime 给每个 Cell |
| `session_zones` | C/L | manager 的实际 Join/Leave 路由；存档可有位置描述，冷恢复不能据此重建在线权限 |
| `online_identities` | L | manager 分配 namespace/epoch；checkpoint 验证后清除 active owners，真实 Join 再准入 |
| `logical_world` | C | v4 新增，可信 factory 按持久逻辑执行域首次绑定；同值幂等，已有值不能换成另一世界。冷恢复保留描述，不能授权写入 |
| `admission_sequence` | C | v4 committed 单调准入流；实际单分区命令、全 tick、顺序 tick、fast movement timer 在修改前编号；parallel tick 按 ZoneKey 先编号，耗尽整批拒绝 |
| `last_admission` | C | v4 committed 的最后一个 action ID 与稳定二进制命令摘要；只保留一个收据，避免增长，不代替 P3 的持久去重 ledger。序列与world必须一致 |
| `map_random_master` | C | v4 mandatory、version 1 的32字节安全随机主密钥，Debug遮蔽、checkpoint/root绑定、fork/cold精确保留；新地图用HMAC(master, 固定domain+旧ZoneKey)派生seed，host/epoch不参与。既有地图v8流不重置；旧v2/v3在全部原root和路由校验后fresh一次 |

## ZoneRuntime：全部直接字段

| 字段 | 分类 | 当前序列化/重建与后续边界 |
| --- | --- | --- |
| `five_class_skills` | L/E | v7 checkpoint 未覆盖；包括 `last_now_ms`、`actors`、`pending`、`explosions`、`bleeds`、`clones`、`traps`、`vampires`、`cleanup_outbound`。可信 live fork 保留，不能冷恢复为在线技能；独立 writer 前需完整 owner 转移合同 |
| `five_class_pvp` | L/E | v7 未覆盖其 pending 命中；同上，不能用冷快照代替 live 交接 |
| `combat_random` | C | v8 新增 map 唯一 HMAC 随机流，wire 为 `version:1`、`seed:[u8;32]`、`draws:u64`；seed/游标均持久并参与 canonical，可信 fork 保留，同一已准入执行按相同顺序消费；不可把 seed 发给普通客户端。冷恢复仍不恢复在线权限 |
| `visibility_now_ms` | L | 当前在线友方陷阱显示时钟；不能冷恢复线上行动权限 |
| `mining` | C | v7 committed；矿点余量、regen 时间、sequence 和 effects 整体保留 |
| `conquest` | K/L | 外层可信领地记录投影；v7 未覆盖，必须按权威记录和版本重新绑定 |
| `intelligent_creatures` | C/L | v4+ committed host；含实体状态，任何内部在线证明恢复仍受清权策略限制 |
| `intelligent_creature_intents` | C/E | v4+ committed 待拾取意图；保留原 owner、目标、顺序，不在各 Cell 重发 |
| `intelligent_creature_operations` | C/E | v4+ committed 待执行操作；不能在切片时遗漏或多执行 |
| `key` | C | 旧 ZoneKey 字节与业务作用域不变，物理地址禁止写入 |
| `collision` | K/C | runtime checkpoint 不直接保存碰撞块，按 map/key 重建；canonical 包含碰撞摘要状态，开门状态另由 open_doors 恢复 |
| `journey_evidence_enabled` | K | 构造时可信 cadence 配置；不得在迁移时默认开启产生额外奖励证据 |
| `players` | C/L | v7 committed 玩家主体；在线能力在冷恢复清除，子字段见下表 |
| `online_presence` | L | v4+ 可参与当前摘要；冷恢复清权，不从 bytes 制造 Node |
| `managed_online_identity` | L | 当前 manager 管理方式；不属于持久业务权威 |
| `local_online_identities` | L | 独立 runtime 的准入源；namespace/epoch 不能从可导入快照恢复为在线 owner |
| `detached_ground_drop_custody` | C/L | v6+ committed 结算 custody；恢复验证而非重新生成收益身份，在线 owner 必须符合清权/重新绑定合同 |
| `issued_native_monster_awards` | C/L | committed 已签发奖励与 owner 描述；不得重复签发、不得用旧在线描述授权新实体 |
| `objects` | C | retained object + packet frame、health、mana、expires_at_ms、buffs 全保留 |
| `dead_object_ids` | C | 尸体位置/方向墓碑；不因 Cell 变更丢弃 |
| `revived_object_ids` | C | 已复活集合，与死亡/移除顺序一起保留 |
| `removed_object_ids` | C | 移除墓碑必须参与摘要，避免对象复现 |
| `harvested_object_ids` | C | 尸体采集状态；不得重置生成第二份掉落 |
| `native_monsters` | C | 完整生命、incarnation、AI、仇恨/归属、buff/deadline、控制/移动/攻击状态、已滚落物、drop seed；子字段随唯一实体 owner |
| `native_monster_respawns` | C | v4+ committed 重生计划/绝对 deadline；同地图跨 Cell 不重排或重计时 |
| `pending_native_hits` | E | 怪物延迟命中、fire bounce、practice/journey 草稿；原到期顺序和来源身份保留 |
| `pending_native_projectiles` | E | 飞行物源/目标和 ready_at_ms；不能分别随源/目标复制 |
| `pending_native_player_hits` | E | 玩家待命中、攻击类型和目标 ID；独立 writer 前补齐稳定 action 与完整目标生命证明 |
| `pending_native_player_heals` | E | 待治疗和 journey 草稿；不能因目标搬迁重复投递并结算 |
| `pending_native_purifications` | L/E | 明确 live-only、无 Deserialize；caster/target 真实在线证明只在 live fork/可信 transfer 保留 |
| `pending_native_summons` | E | 所有者/生命、召唤限制、目标、寿命、技能、坐标和草稿一起保留 |
| `pending_native_ground_spells` | E | 地面技能位置/对象 ID、phase、next_damage_at_ms、expires_at_ms、原间隔和 trap 状态由唯一 map effect owner 持有 |
| `ground_drops` | C | 原 object_id、generation、payload digest、owner 和消失时间不变；不得从呈现 packet 重造 |
| `claimed_ground_drops` | C/G | Zone 认领与既有账户结算边界共同维护；保留 claim 与幂等键 |
| `next_ground_drop_generation` | C | committed 单调 generation，耗尽拒绝，跨 Cell 不重置 |
| `next_ground_drop_claim_id` | C | committed 单调 claim ID，重放复用 |
| `occupancy` | D | 从所有真实玩家位置重建；重复占格必须拒绝恢复，Ghost 不占格 |
| `npc_teleport_config` | K | 可信配置恢复；同地图 Cell crossing 不调用跨地图 NPC 传送流程 |
| `open_doors` | C | committed 门状态与自动关闭绝对时间；重建 collision 后应用 |
| `hazard` | C/E | committed lightning/fire 开关、伤害、next_ms、strike 计数；地图事件只能执行一次 |
| `hell_world` | C/E | 专属 encounter/随机/待执行状态整体归地图，未另立切片合同前不拆 |
| `tree_queen_world` | C/E | 同上，不能各 Cell 独立执行 Boss 阶段 |
| `pet_special_world` | C/E | 特殊宠物状态、效果整体保留；不由 Ghost 推进 |
| `next_monster_incarnation` | C | 全图分配器，恢复至少不低于现有实体最大 incarnation；独立 Cell 需显式不重叠分配合同 |
| `entity_combat` | C/E | committed 战斗/拥有宠物状态，包括对实际生命的引用、延迟动作及回执；整体保留直到有专门切片合同 |
| `horned_encounter_world` | C/E | Boss encounter 先保留地图唯一 owner |
| `native_periodic_player_poisons` | E | 持续伤害来源、目标生命与原 next/expire 时钟归 map effect owner |
| `player_grid` | D | 从真实玩家位置重建，不能包含第二个可写 player |
| `object_grid` | D | 从 retained objects 位置重建，怪物移动后同步派生索引 |
| `ecs` | D | 当前 players 的 ECS mirror；不是第二份权威，不能从 Ghost 恢复可写实体 |
| `next_object_id` | C | 当前全图分配器；同图所有类型不得因多 Cell 从同一个起点分配 |

## ZonePlayer：全部直接字段

下表列出所有直接字段；集合/结构内部继承该行 owner。这里的 committed 指玩家主体当前
参与 canonical/checkpoint，不表示可以在冷恢复后保留在线身份。

| 字段 | 分类 | 合同 |
| --- | --- | --- |
| `experience_profile`、`mentor_bank` | C/G | 可信账户/共享关系投影；结算由既有经济 owner 完成 |
| `session_id`、`account_id`、`character_index`、`object_id`、`name` | C/L | 角色与实体身份；描述可保留，当前 online admission 必须重新验证 |
| `name_colour_argb`、`class`、`gender`、`level` | C | 角色属性与真实呈现 |
| `hp`、`max_hp`、`mp`、`dead`、`life_generation` | C | 唯一 Zone 生命状态；复活改变 life，Cell crossing 不改变 |
| `position`、`direction` | C | 真实占格/AOI/保存同一来源；失败不能先公开移动成功 |
| `hair`、`light`、`weapon`、`weapon_effect`、`armour` | C/G | 可信装备投影与呈现；不是可写背包 |
| `poison`、`native_status_poison`、`native_status_poison_expires_at_ms`、`native_status_poison_deadlines` | C/E | 保留控制/持续状态与绝对时限，不能恢复成完整新持续时间 |
| `vital_receipt_sequence`、`mana_receipt_sequence`、`five_class_practice_sequence`、`owner_health_sequence`、`journey_event_sequence` | C | 原顺序/去重水位独立保留；owner/生命代次校验不省略 |
| `pending_owner_health_changes` | L | serde skip 的命令内呈现通知；不能由冷 checkpoint 重放到新连接 |
| `hidden`、`sneaking`、`effect`、`wing_effect`、`mount_type`、`riding_mount`、`fishing`、`transform_type`、`level_effects` | C | 行为与可见状态由实体 owner 产生，Ghost 只观察 |
| `element_orb_effect`、`element_orb_level`、`element_orb_max` | C | 五职业状态与呈现不能在 Cell 之间分叉 |
| `visible_object_ids` | C/D | 当前 committed 缓存也是协议 diff 基线；固定 Cell 切片必须在完整邻区上统一重建，不能清空导致额外消失/重现 |
| `movement_actions`、`last_seen_move_seq` | C/L | 当前准入移动队列/序号；断线按原 teardown 取消未消费意图，迁移不等于断线 |
| `movement_ready_at_ms`、`run_step_until_ms`、`shout_ready_at_ms`、`next_attack_ready_at_ms`、`next_spell_ready_at_ms`、`magic_ready_at_ms` | C | 原绝对 deadline 和阶段顺序，迁移不得缩短或重置 |
| `flaming_sword_armed`、`fatal_sword_armed`、`flaming_sword_ready_at_ms` | C | 已准入剑技与冷却保持 |
| `last_damaged_at_ms`、`native_struck_ready_at_ms`、`last_regen_at_ms` | C | 受击呈现、回血与战斗计时器不可混用 |
| `chat_profile`、`combat_stats`、`combat_state` | C/G | 外层可信同步、Zone 实际判定；缺失 combat_state 保持 fail closed |
| `buffs`、`reincarnation_offer` | C/E | 包括施法者、ready/expires、requested、will_succeed 等所有子字段；不凭重试重滚结果 |

## Gateway shared Zone：全部直接字段

个人 Session/背包、角色保存仍由外层管理。以下字段在固定 Cell 下仍只有一个 gateway
适配 owner；不能将整个 SharedInProcessZoneState 复制给每个 Cell。

| 字段 | 分类 | 合同 |
| --- | --- | --- |
| `warm_pet_snapshots` | L/G | 刻意不进 checkpoint；保留 warm Node 行为，不伪装成持久 PetInfo |
| `experience_authority` | K/G | 可信配置重新绑定，不能从世界 bytes 导入发布权 |
| `next_zone_object_id` | C/G | shared world 全图对象分配，Cell 不能各自重置 |
| `next_live_outbound_registration_id` | C/L | 旧 transport stream 描述不构成可恢复连接 |
| `zone_cadence_tick_count` | D | test-only 观察计数 |
| `next_economy_source_sequence` | C/G | 持久准入序号；不得拿 placement epoch 替代或迁移重置 |
| `zone_manager` | C/L | 业务状态/在线能力边界见上文 |
| `zone_sessions`、`zone_session_keys` | C/L | 真实 Join 路由及其反向映射；冷恢复清会话、保留必要未完成结算校验 |
| `pending_zone_packets`、`pending_zone_transforms`、`pending_zone_shout_consumes` | C/G | 原协议与保存 drain 顺序；最终 Zone transform 覆盖旧私有镜像 |
| `pending_zone_ground_drop_claims`、`pending_zone_monster_kill_awards`、`pending_zone_monster_kill_proofs`、`pending_zone_player_kills` | C/G | 原精确来源/幂等键、生命代次、内容摘要和发布状态不变 |
| `pending_zone_player_damages`、`pending_zone_player_heals`、`vital_receipt_progress` | C/G | 真实 Zone receipts，不能重放成第二次 HP/死亡处罚 |
| `pending_zone_owner_health` | L | 在线呈现队列刻意不进冷快照 |
| `pending_zone_magic_practice`、`magic_practice_progress`、`pending_zone_journey_events`、`journey_event_progress` | C/G | 技能练习和任务事件独立回执流，不能合并成另一奖励通道 |
| `teardown_fences` | C/L | 原保存屏障；恢复按完整 world/账户恢复策略，不当作新写能力 |
| `teardown_vital_fence_generations`、`next_teardown_vital_fence_generation` | L | 当前 exact teardown drain 的进程内能力，不序列化 |
| `live_zone_outbounds` | L | 当前传输注册与投递 owner，不从快照伪造连接 |
| `ranking_inspect_projections` | L/G | 当前 owner 装备只读视图；不变成离线存档或新背包 |
| `live_zone_player_ids`、`departed_live_zone_player_ids` | D/L | 退场呈现识别缓存，不构成实体权威 |
| `players`、`maps` | C/G | gateway 地图/玩家适配投影；真实移动/生命仍读取 Zone |
| `trade_links`、`trade_offers`、`pending_trade_deliveries`、`pending_trade_rollbacks` | C/G | 原共享交易事务/回滚 owner，Cell crossing 不复制交易 |
| `unresolved_ground_drop_settlements`、`unresolved_trade_settlements` | C/G | 不确定提交保留 exact source context/receipt；禁止换键重试或猜测退款 |
| `pending_rental_invites`、`pending_rental_cancels`、`rental_item_offers`、`rental_fee_offers`、`pending_rental_deliveries` | C/G | 原租赁事务与实际物品 custody |
| `npc_saved_values`、`npc_random_seed` | C/G | 地图/NPC 脚本状态和随机状态唯一持有，分区不各执行一次 |

## 固定 Cell 的晋级限制

### Manager 可变入口的行为身份边界

`StableActionId` 唯一使用 manager 持久准入流。RPC journal 和账户经济 source sequence
仍是各自原事务字段，不同时映射为同一个 world/Zone 的另一条 StableActionId 流。
最后一个准入收据是有界观察元数据，不记录完整输入日志，也不替代旧收益幂等键。

| 入口 | 身份与提交边界 |
| --- | --- |
| `handle` / `handle_for_key` / `handle_for_partition` / `join` | 最终到一次 `handle_single_partition` 准入；错误 partition 或 session route 在编号前拒绝 |
| `join_with_appearance` | Join 同一次准入内校准外观，不把原 ObjectPlayer 装饰再变为另一顶层动作 |
| `tick_all` / `tick_all_sequential` / `tick_pending_movement` | mutation 前按 ZoneKey 排序准入；并行 worker 不决定序号，顺序 fallback 不二次编号 |
| `refresh_online_pets_at` | 独立公开调用用 version 1 `online_pet_refresh`；tick 内用 private 已准入路径，召回/死亡/跨图保留原 pet life |
| `commit_owner_vitals` | version 1 `owner_vitals_commit` 记录可信池提交；原 gateway prepared life 仍校验，呈现 cursor 不能授权 HP 修改 |
| `restore_player_saved_pets` | 校验原 PetInfo 后 version 1 `restore_saved_pets`，完整 typed snapshot 入摘要；不创造在线能力 |
| `sync_intelligent_creature` / `request_intelligent_creature_pickup` | version 1 `intelligent_creature_sync` / `intelligent_creature_pickup_request`，完整 typed 参数；实际意图/目标继续由原逻辑校验 |
| `commit_mining_swing` | version 1 `mining_swing_commit`，原可信 swing 全字段入摘要；不添加 Deserialize，不改变矿点 sequence、before/after CAS 或个人交易身份 |
| `spawn_world_event_monster` | version 1 `world_event_monster_spawn`，完整 spawn 与逻辑时钟；同图 ID/incarnation 仍由原实体规则校验 |
| `prepare_mining_swing` | 只读准备，不造已提交动作；事前检查可用准入槽。外层保持同一写锁并在个人提交前检查整次复合操作的槽数，不能在账户提交后才发现准入耗尽 |
| `mirror_player_pet_earned_steps` | 既有 `ZonePetExperienceAdmission` 的真实 online/pet incarnation 与 before experience/level CAS；只能镜像已持久源的 steps，不重造经验来源或 action |
| `apply_conquest_projection` | 原持久 conquest record revision、membership epoch 和可信配置投影；不从每个 Cell 再执行一份领地事件 |
| `update_experience_profile` | 原账户/关系/profile 来源投影；刷新 shared presence 是派生维护，不是发放经验的新指令 |
| `acknowledge_issued_monster_award` / `acknowledge_owned_pet_kill` | 复用原奖励 receipt、死亡/incarnation/序号；只确认已有签发，不产生新奖励 |
| `forget_detached_ground_custody` / `detach_ground_drop_claim` / `detach_all_ground_drop_claims` / `restore_detached_ground_drop_claim` | 复用原精确 claim ticket、generation、payload digest 和 custody；只交接/恢复该原收益事务 |
| `settle_intelligent_creature_operation` / `settle_intelligent_creature_pickup` | 原 operation_id 或已提交 drop claim 的 actor/drop 匹配；先有原账户提交再 ack，不能据重试造第二份收益 |
| `drain_intelligent_creature_operations` / `drain_intelligent_creature_pickup_intents` | 交付原 owned operation/intent，保留其他人的顺序；不是新游戏准入 |
| `leave_all_sessions` | 先整批预检准入空间，不足完全不动；每个真实 Leave 经已有 handle 准入，只返回真实移除数，不额外分配一个全局 Logout ID |
| `clear_sessions_for_cold_world` | 专供冷 checkpoint 转换，使用原 Leave 清场规则但不造 live 准入，不改已持久的world/sequence/receipt；不能用于实际玩家Logout |
| `broadcast_world_event_message` | 只读发送现有地图消息；不存在地图直接无接收者返回，不为聊天实例化地图/随机流/目录；不产生生命/占格/物品/经济决定 |
| `install_empty_zone` | 可信 bootstrap/fixture，只能在无 active session 时替换配置 runtime；不得用于在线 Cell 迁移 |
| `bind_logical_world` / checkpoint restore / live fork | 初始化、严格恢复或可信比较；不是新的玩家动作，亦不恢复 owner lease |

新建地图的随机主密钥必须先进入共同的版本化 base，再重放其后的增量；没有共同 base 的
两个独立 fresh manager 不保证相同随机流。旧快照升级产生新密钥/地图随机流后，应由真实
owner 发布升级后的 base，不能让 active 和 standby 各自独立升级旧快照再声称完全确定性。
共同 base 保证随机业务状态，不允许反过来从快照恢复在线 namespace/owner 权限。

### 快照兼容与回退

当前配对写入版本为 manager v4、runtime/canonical v8、gateway factory v3；旧 reader
会拒绝这些新版本。回退需使用升级前已验证且相互一致的备份与恢复流程，不能删除新字段、
把 version 改回旧值或重新签一份旧摘要来冒充降级。新版本已产生的实际账户/经济提交仍须
按原事务收据保留，不能仅回滚世界快照导致已发收益重新结算。

legacy manager v2/v3 先执行原分层校验（v3 含原 manager 摘要），验证所有 runtime 与路由
后才生成一次新 master；runtime v1–v7 先按各原摘要域验证再生成一次新业务随机流。
这次升级必须由真实 owner 输出含新 seed/游标的共同版本化 base，再恢复其后的增量流。
不支持 active/standby 各自独立升级旧数据、或没有共同 base 的完整 host journal 重放，
然后声称业务随机与所有在线权威状态完全确定。冷 base 清除在线能力的规则仍然生效。

1. 第一阶段固定 Cell 是完整 Zone 上的空间视图/索引与单提交器，不改变现有冷恢复清权、
   游戏阶段、随机调用次序、ID、收据或个人事务。完整 online runtime 的可信 live fork 与
   checkpoint deserialize 是不同操作，不能相互替代。
2. 原 `owned_pet_combat.rs` 中击杀 curse roll、ElectricShock 命中/震慑/驯服/狂暴时长
   直接使用 `OsRng`，不能仅凭相同输入 trace 保证确定性。当前改造以 v8 `combat_random`
   统一该随机流，晋级必须有真实对应技能/死亡回执的 fork 与恢复对照结果。
   身份 namespace 的安全随机与游戏随机不同，继续保留安全随机，不改成可预测身份。
3. 同一准入 trace、逻辑时钟和随机流状态才可比较固定 Cell 单提交器；随机调用顺序仍须
   相同。尚未接入可重放随机流的路径必须显式列作不可比较，不能静默忽略差异。
4. P3/P5 交接应覆盖上表每项 C/E/L 的实际迁移闭包、待处理消息、随机、收据、发布水位与
   失败恢复。任何未定义转移的 live-only 技能/encounter 继续绑定原地图 owner。

### P2 完整在线参考摘要的实现约束（尚未实施）

当前 51 个 runtime 直接字段中，`occupancy/player_grid/object_grid/ecs` 四项派生镜像须
重建并单独校验；其余 47 项业务、在线或配置状态必须进入独立的在线摘要。CP8 不能替代
完整在线状态。`collision` 包括随门开关变化的阻挡格，NPC 配置及 journey 开关也须保留，
不能在重组时重新读取当前默认配置。`visible_object_ids` 是下一次协议 diff 的基线，
不能作为普通缓存清空。

需补齐五职业技能、PvP、conquest、待到期 purification 及健康通知的私有 Serialize-only
投影；不增加导入在线权限的 Deserialize。`ZonePlayer.pending_owner_health_changes`
被现有 serde 跳过，该缺口同时存在于 `players` 和
`entity_combat.owned_pet.detached_masters`，必须统一覆盖。技能清理队列中的
`ZoneOutbound` 要穷尽匹配全部 variant，保留收件人和 packet 原序；实际封包编码失败须
返回错误，不使用 Debug 或空值代替。归档对象只可信克隆 typed 状态，不提供生产激活入口。

生命代次、随机 seed/游标、各怪物独立 RNG、全部绝对时钟、待处理 Vec/VecDeque 顺序、
未确认收据及 claim/UID/custody 身份均须保留。以穷尽解构且无 `..` 的字段分类约束新增
runtime 字段；在线摘要与有序 outbound 摘要分别比较，不靠同一冷存档 root 推断输出相同。
未来 CellCoord 或 ZoneKey 是结构化键，JSON 切片应复用 manager 的有序 entry-array
表示，而非把结构键直接放入 JSON map 或转成 Debug 字符串。布局与派生镜像不进入布局无关
业务摘要，其一致性另作每步断言。这些是 P2 晋级要求，静态盘点不构成已实现或验收证据。

源码入口：[runtime](../apps/simulation/src/runtime/zone/runtime.rs)、
[checkpoint](../apps/simulation/src/runtime/zone/runtime/checkpoint.rs)、
[manager](../apps/simulation/src/runtime/zone/manager.rs)、
[player/types](../apps/simulation/src/runtime/zone/types.rs)、
[gateway shared state](../apps/gateway/src/routing.rs)、
[内部身份合同](../apps/simulation/src/runtime/zone/contract.rs)。
