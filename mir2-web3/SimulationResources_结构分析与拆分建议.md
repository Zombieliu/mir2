# `SimulationResources` 结构分析与拆分建议

> 目标：在不改变现有玩法行为、不破坏 `SimulationSession` 对外 API 的前提下，将当前过于庞大的 `SimulationResources` 拆成更清晰、可维护、适合 MMO 服务端演进的运行时结构。

---

## 1. 当前问题概述

当前 `SimulationResources` 几乎承担了整个 runtime 的所有职责，包括：

- 配置
- 地图状态
- 账号状态
- 当前角色
- 玩家位置
- 玩家血蓝
- 经验金币
- 仓库 / 背包 / 装备
- 任务
- 技能
- NPC flag
- NPC 脚本变量
- BuyBack / UsedGoods
- Buff
- Stage5 系统状态
- 当前 NPC 对话
- pending combat
- pending monster spawn
- tick
- object id 分配器

这说明它已经不只是一个普通的 Bevy `Resource`，而是变成了：

```text
SimulationResources =
    Session State
  + Player State
  + Map State
  + Inventory State
  + Quest State
  + NPC State
  + Combat Queue
  + Spawn Queue
  + Persistence Snapshot
  + Runtime Clock
  + Object Id Allocator
```

短期看，这种写法非常适合快速做 PoC 和 1:1 复刻行为，因为所有状态都集中在一个地方，Codex 也容易继续往里补逻辑。

但长期看，它会带来非常明显的问题：

1. **状态边界不清晰**
2. **多人 MMO 扩展困难**
3. **函数之间耦合严重**
4. **测试难度增加**
5. **并发和分区困难**
6. **容易形成巨型单文件 / 巨型 Resource**
7. **后续接 WebSocket、Redpanda、ClickHouse、见证网络时会很难拆事件**

---

## 2. 为什么 `SimulationResources` 太肥是一个严重问题

### 2.1 它混合了 Session 状态和 World 状态

当前结构里既有账号和角色信息，也有地图、怪物、掉落、pending spawn、tick 等运行时世界状态。

但是在正式 MMO 服务端中，这两者应该分开：

```text
Session:
- 当前连接
- 账号 ID
- 角色 ID
- 语言
- 登录状态
- controlled entity

MapInstance:
- ECS World
- 地图信息
- 碰撞数据
- 玩家实体
- 怪物实体
- NPC 实体
- 掉落实体
- AOI Grid
- tick
```

如果 Session 自己持有一个完整世界，那么它更像是“单人模拟器”，而不是多人共享地图服务器。

---

### 2.2 它混合了持久化状态和运行时状态

例如：

- 背包
- 装备
- 技能
- 任务
- NPC flag
- npc saved values
- buy back items
- used goods items
- stage5 systems

这些更接近角色存档 / 数据库状态。

而下面这些更接近运行时状态：

- pending combat actions
- pending monster spawns
- active npc dialog
- active npc service
- tick
- next drop object id
- next runtime monster object id

两类状态放在一起，会导致后面存档、回滚、事件重放、跨服迁移、数据修复都变麻烦。

---

### 2.3 它让所有系统都依赖一个巨型全局对象

如果大量函数都是这样：

```rust
let mut resources = world.resource_mut::<SimulationResources>();
```

那么每个系统都可能读写所有状态。

这会导致：

- 很难知道某个函数真正修改了什么
- 很难做细粒度测试
- 很难拆模块
- 很难并行运行系统
- 很难避免 borrow 冲突
- 很难保证状态一致性

更好的方式是把 Resource 拆成多个小 Resource，每个系统只读写自己需要的部分。

---

## 3. 推荐拆分目标

建议将 `SimulationResources` 拆成以下几个更清晰的 Resource。

---

## 4. Resource 拆分方案

### 4.1 `RuntimeConfigResource`

负责全局配置和静态启动配置。

```rust
pub struct RuntimeConfigResource {
    pub config: SimulationConfig,
}
```

包含：

- `config`

说明：

`SimulationConfig` 不应该和玩家状态、地图状态、任务状态混在一起。它是启动配置，不是运行时业务状态。

---

### 4.2 `SessionResource`

负责当前连接 / 登录会话状态。

```rust
pub struct SessionResource {
    pub language: LanguageCode,
    pub version_verified: bool,
    pub account_id: Option<String>,
    pub characters: Vec<CharacterRecord>,
    pub selected_character: Option<CharacterRecord>,
}
```

包含：

- `language`
- `version_verified`
- `account_id`
- `characters`
- `selected_character`

说明：

这部分代表“谁在连接服务器”，不代表“世界里发生了什么”。

---

### 4.3 `PlayerRuntimeResource`

负责当前玩家在世界里的运行时状态。

```rust
pub struct PlayerRuntimeResource {
    pub position: Point,
    pub direction: MirDirection,
    pub vitals: PlayerVitals,
    pub experience: i64,
    pub max_experience: i64,
    pub gold: u32,
    pub credit: u32,
}
```

包含：

- `player_position`
- `player_direction`
- `player_vitals`
- `experience`
- `max_experience`
- `gold`
- `credit`

说明：

这是角色当前状态。后续多人化时，这些字段更适合变成玩家 Entity 上的 Components，而不是全局 Resource。

例如：

```rust
#[derive(Component)]
pub struct Player;

#[derive(Component)]
pub struct Position(pub Point);

#[derive(Component)]
pub struct Facing(pub MirDirection);

#[derive(Component)]
pub struct PlayerVitals {
    pub hp: i32,
    pub max_hp: i32,
    pub mp: i32,
}
```

短期可以先放 Resource，长期建议迁移到 Entity Component。

---

### 4.4 `MapRuntimeResource`

负责当前地图实例状态。

```rust
pub struct MapRuntimeResource {
    pub current_map: MapInformation,
    pub map_region_bounds: MapBounds,
    pub blocked_cells: BTreeSet<(i32, i32)>,
    pub closed_door_cells: BTreeSet<(i32, i32)>,
    pub conquest_wars: BTreeMap<i32, bool>,
}
```

包含：

- `current_map`
- `map_region_bounds`
- `blocked_cells`
- `closed_door_cells`
- `conquest_wars`

说明：

地图状态应该属于 MapInstance。

未来正式 MMO 架构中，应该是：

```text
MapInstance
  - World
  - MapRuntimeResource
  - AOI Grid
  - Collision
  - Players
  - Monsters
  - NPCs
  - Drops
```

而不是每个玩家 Session 都有一套完整地图状态。

---

### 4.5 `InventoryResource`

负责背包、腰带、仓库、装备。

```rust
pub struct InventoryResource {
    pub inventory_items: Vec<ItemState>,
    pub belt_items: Vec<ItemState>,
    pub storage_items: Vec<ItemState>,
    pub equipment_items: Vec<EquipmentState>,

    pub storage_size: u16,
    pub has_expanded_storage: bool,
    pub expanded_storage_expiry_time_binary_datetime: i64,
    pub expanded_storage_expiry_notice_pending: bool,

    pub storage_unlocked: bool,
    pub storage_sent: bool,
    pub storage_has_password: bool,
    pub storage_password_last_set_binary_datetime: i64,
}
```

包含：

- `inventory_items`
- `belt_items`
- `storage_items`
- `equipment_items`
- `storage_size`
- `has_expanded_storage`
- `expanded_storage_expiry_time_binary_datetime`
- `expanded_storage_expiry_notice_pending`
- `storage_unlocked`
- `storage_sent`
- `storage_has_password`
- `storage_password_last_set_binary_datetime`

说明：

背包、装备、仓库是独立的领域。它应该被 inventory / equipment 系统管理，而不应该让 combat、npc、map 等系统随便改。

后续可以继续细分为：

```text
InventoryResource
EquipmentResource
StorageResource
```

但第一阶段可以先合并为一个 `InventoryResource`。

---

### 4.6 `QuestResource`

负责任务状态。

```rust
pub struct QuestResource {
    pub quests: Vec<QuestState>,
}
```

包含：

- `quests`

说明：

任务状态应该独立出来。
战斗系统击杀怪物后不应该直接改 `quests`，更好的方式是发事件：

```rust
GameEvent::MonsterKilled {
    player_entity,
    monster_key,
}
```

然后由 Quest 系统监听事件并更新任务进度。

---

### 4.7 `SkillResource`

负责技能状态。

```rust
pub struct SkillResource {
    pub skills: Vec<SkillState>,
}
```

包含：

- `skills`

说明：

技能冷却、技能等级、技能学习状态都应该属于 Skill 系统。

战斗系统可以查询 SkillResource，但不应该把技能、Buff、装备、任务全部混在一个 Resource 里。

---

### 4.8 `BuffResource`

负责玩家身上的 Buff。

```rust
pub struct BuffResource {
    pub buffs: Vec<BuffState>,
}
```

包含：

- `buffs`

说明：

Buff 会影响：

- 攻击
- 防御
- 移动
- 技能
- 中毒
- 冰冻
- 麻痹
- 眩晕

因此它应该独立存在，并提供清晰的查询函数：

```rust
pub fn total_attack_bonus(&self) -> i32;
pub fn total_defence_bonus(&self) -> i32;
pub fn has_status(&self, key: &str) -> bool;
pub fn tick_expired(&mut self, current_tick: u64);
```

---

### 4.9 `NpcStateResource`

负责 NPC 相关状态。

```rust
pub struct NpcStateResource {
    pub npc_flags: Vec<NpcFlagState>,
    pub npc_variables: Vec<(String, String)>,
    pub npc_saved_values: Vec<CrystalNpcSavedValue>,
    pub npc_script_diagnostics: Vec<CrystalNpcScriptDiagnostic>,
    pub npc_buy_back_items: Vec<NpcBuyBackState>,
    pub npc_used_goods_items: Vec<NpcUsedGoodsState>,
    pub active_npc_dialog: Option<ActiveNpcDialogState>,
    pub active_npc_service: Option<ActiveNpcServiceState>,
}
```

包含：

- `npc_flags`
- `npc_variables`
- `npc_saved_values`
- `npc_script_diagnostics`
- `npc_buy_back_items`
- `npc_used_goods_items`
- `active_npc_dialog`
- `active_npc_service`

说明：

NPC 脚本系统通常非常复杂，不应该和 combat、movement、inventory 全部挤在一个 Resource 里。

尤其是：

- flag
- variables
- saved values
- diagnostics
- buyback
- used goods
- dialog
- service

这些都属于 NPC Script VM / NPC Interaction 层。

---

### 4.10 `RuntimeQueueResource`

负责延迟执行队列。

```rust
pub struct RuntimeQueueResource {
    pub pending_combat_actions: Vec<PendingCombatAction>,
    pub pending_monster_spawns: Vec<PendingMonsterSpawnAction>,
}
```

包含：

- `pending_combat_actions`
- `pending_monster_spawns`

说明：

pending action 是 runtime scheduler 的一部分，不应该放在玩家存档状态里。

未来可以进一步抽象为：

```rust
pub struct ScheduledActionQueue {
    pub combat: Vec<PendingCombatAction>,
    pub spawns: Vec<PendingMonsterSpawnAction>,
    pub effects: Vec<PendingEffectAction>,
}
```

或者统一成：

```rust
pub enum ScheduledAction {
    Combat(PendingCombatAction),
    MonsterSpawn(PendingMonsterSpawnAction),
    Effect(PendingEffectAction),
}
```

---

### 4.11 `RuntimeClockResource`

负责 tick。

```rust
pub struct RuntimeClockResource {
    pub tick: u64,
}
```

包含：

- `tick`

说明：

tick 是全局时间。它应该独立，方便所有系统只读当前 tick。

---

### 4.12 `ObjectIdAllocatorResource`

负责运行时 object id 分配。

```rust
pub struct ObjectIdAllocatorResource {
    pub next_drop_object_id: u32,
    pub next_runtime_monster_object_id: u32,
}
```

包含：

- `next_drop_object_id`
- `next_runtime_monster_object_id`

说明：

object id 分配器最好不要藏在巨型 Resource 里。
后续如果要支持多地图、多实例、跨服，object id 规则会变复杂。

可以提供方法：

```rust
impl ObjectIdAllocatorResource {
    pub fn next_drop_id(&mut self) -> u32;
    pub fn next_runtime_monster_id(&mut self) -> u32;
}
```

---

### 4.13 `Stage5SystemsResource`

负责 Stage5 系统。

```rust
pub struct Stage5SystemsResource {
    pub state: Stage5SystemsState,
}
```

包含：

- `stage5_systems`

说明：

Stage5 看起来包含拍卖、邮件、交易、Hero 等偏后期系统。
它不应该和基础移动、战斗、地图混在一起。

后续建议继续拆成：

```text
AuctionResource
MailResource
TradeResource
HeroResource
```

但第一阶段可以先包一层。

---

### 4.14 `GroupResource`

负责组队 / 可见组员状态。

```rust
pub struct GroupResource {
    pub group_member_object_ids: Vec<u32>,
}
```

包含：

- `group_member_object_ids`

说明：

组队以后会变复杂：

- 队长
- 队员
- 经验分配
- 掉落归属
- 小地图显示
- 聊天频道
- 副本进入权限

所以建议独立出来。

---

### 4.15 `PlayerPermissionResource`

负责特殊权限 / 临时权限。

```rust
pub struct PlayerPermissionResource {
    pub unlock_curse: bool,
    pub free_map_shout: bool,
    pub free_server_shout: bool,
}
```

包含：

- `unlock_curse`
- `free_map_shout`
- `free_server_shout`

说明：

这些是玩家临时权限 / 特殊状态，和背包、地图、战斗没有强绑定。

---

### 4.16 `PotionRecoveryResource`

负责持续恢复药水状态。

```rust
pub struct PotionRecoveryResource {
    pub pending_pot_health_amount: i32,
    pub pending_pot_mana_amount: i32,
}
```

包含：

- `pending_pot_health_amount`
- `pending_pot_mana_amount`

说明：

这是药水恢复队列 / 延迟回血蓝状态，应该由 item/potion/buff 系统管理。

---

## 5. 拆分后的整体结构

第一阶段拆分后，资源大概是这样：

```text
RuntimeConfigResource
SessionResource
PlayerRuntimeResource
MapRuntimeResource
InventoryResource
QuestResource
SkillResource
BuffResource
NpcStateResource
RuntimeQueueResource
RuntimeClockResource
ObjectIdAllocatorResource
Stage5SystemsResource
GroupResource
PlayerPermissionResource
PotionRecoveryResource
```

也可以画成：

```text
SimulationSession
  └── HeadlessRuntime / Bevy World
        ├── RuntimeConfigResource
        ├── SessionResource
        ├── PlayerRuntimeResource
        ├── MapRuntimeResource
        ├── InventoryResource
        ├── QuestResource
        ├── SkillResource
        ├── BuffResource
        ├── NpcStateResource
        ├── RuntimeQueueResource
        ├── RuntimeClockResource
        ├── ObjectIdAllocatorResource
        ├── Stage5SystemsResource
        ├── GroupResource
        ├── PlayerPermissionResource
        └── PotionRecoveryResource
```

---

## 6. 不建议第一阶段就做的事

### 6.1 不要一上来就改玩法逻辑

第一阶段目标应该是：

```text
只拆结构，不改行为
```

不要顺手重写 combat、movement、NPC、掉落逻辑。

---

### 6.2 不要立刻把所有玩家状态都改成 Component

虽然长期看，玩家状态应该 Entity 化：

```rust
Player
Position
Facing
Vitals
Inventory
Equipment
QuestLog
```

但现在如果一次性做，改动会太大。

建议阶段性处理：

1. 先从巨型 Resource 拆成小 Resource
2. 再把玩家运行时状态逐渐迁移到 Player Entity 上
3. 最后再改成真正多人 MapInstance 模型

---

### 6.3 不要立刻改成多地图多 World

这一步很重要，但不适合和 Resource 拆分同时做。

建议顺序：

```text
Step 1: 拆 Resource
Step 2: 拆文件模块
Step 3: 加 GameEvent
Step 4: 引入 MapInstance
Step 5: Session 不再持有完整 World
```

---

## 7. 推荐迁移顺序

### 第一步：只创建新 Resource 类型，不移动逻辑

先在 `resources.rs` 里定义这些新结构：

```rust
pub struct SessionResource { ... }
pub struct PlayerRuntimeResource { ... }
pub struct MapRuntimeResource { ... }
pub struct InventoryResource { ... }
```

暂时不改业务逻辑，只准备结构。

---

### 第二步：修改 `SimulationResources::new`

把原来初始化 `SimulationResources` 的逻辑，拆成多个初始化函数：

```rust
fn init_session_resource(config: &SimulationConfig) -> SessionResource;
fn init_player_runtime_resource(config: &SimulationConfig) -> PlayerRuntimeResource;
fn init_map_runtime_resource(config: &SimulationConfig) -> MapRuntimeResource;
fn init_inventory_resource() -> InventoryResource;
```

然后在 `SimulationSession::new` 里分别 insert：

```rust
app.insert_resource(RuntimeConfigResource::new(&config));
app.insert_resource(SessionResource::new(&config));
app.insert_resource(PlayerRuntimeResource::new(&config));
app.insert_resource(MapRuntimeResource::new(&config));
```

---

### 第三步：从低风险字段开始迁移

优先迁移低耦合字段：

1. `tick` → `RuntimeClockResource`
2. `next_drop_object_id` / `next_runtime_monster_object_id` → `ObjectIdAllocatorResource`
3. `language` / `version_verified` / `account_id` → `SessionResource`
4. `group_member_object_ids` → `GroupResource`
5. `unlock_curse` / `free_map_shout` / `free_server_shout` → `PlayerPermissionResource`

这些字段相对独立，风险低。

---

### 第四步：迁移中等风险字段

然后迁移：

1. `inventory_items`
2. `belt_items`
3. `storage_items`
4. `equipment_items`
5. `quests`
6. `skills`
7. `buffs`

这部分函数引用会比较多，需要配合测试。

---

### 第五步：迁移高风险字段

最后迁移：

1. `npc_flags`
2. `npc_variables`
3. `npc_saved_values`
4. `npc_buy_back_items`
5. `npc_used_goods_items`
6. `active_npc_dialog`
7. `active_npc_service`
8. `pending_combat_actions`
9. `pending_monster_spawns`
10. `current_map`
11. `player_position`

这些字段关联业务逻辑较多，不建议第一批动。

---

## 8. 推荐给 Codex 的提示词

可以直接给 Codex：

```text
Do not change gameplay behavior.

The current `SimulationResources` is too large and contains session state, player state, map state, inventory state, NPC state, quest state, runtime queues, tick, and object id allocation.

Please refactor it into smaller Bevy Resources without changing public APIs or gameplay behavior.

Requirements:
- Keep `SimulationSession` public API unchanged.
- Keep all existing tests passing.
- Do not implement new gameplay features.
- Do not rewrite combat, NPC, movement, inventory, or drop logic.
- Only split state into smaller resources and update references.
- Prefer small incremental commits.
- Run tests after each major migration.

Suggested resources:
- RuntimeConfigResource
- SessionResource
- PlayerRuntimeResource
- MapRuntimeResource
- InventoryResource
- QuestResource
- SkillResource
- BuffResource
- NpcStateResource
- RuntimeQueueResource
- RuntimeClockResource
- ObjectIdAllocatorResource
- Stage5SystemsResource
- GroupResource
- PlayerPermissionResource
- PotionRecoveryResource

Migration order:
1. Move tick to RuntimeClockResource.
2. Move object id counters to ObjectIdAllocatorResource.
3. Move language/account/version fields to SessionResource.
4. Move inventory/equipment/storage fields to InventoryResource.
5. Move quest fields to QuestResource.
6. Move skill fields to SkillResource.
7. Move buff fields to BuffResource.
8. Move NPC fields to NpcStateResource.
9. Move pending actions to RuntimeQueueResource.
10. Move map fields to MapRuntimeResource.

Do not change behavior. This is a structural refactor only.
```

中文版本：

```text
不要改变任何玩法行为。

当前 `SimulationResources` 太大，里面混合了 session 状态、玩家状态、地图状态、背包状态、NPC 状态、任务状态、运行时队列、tick、object id 分配器等。

请把它拆成更小的 Bevy Resources，但不要改变 public API 和玩法行为。

要求：
- `SimulationSession` 的 public API 保持不变
- 所有现有测试必须通过
- 不要实现新功能
- 不要重写 combat、NPC、movement、inventory、drop 逻辑
- 只拆分状态并更新引用
- 尽量小步提交
- 每迁移一大块就运行测试

建议拆成：
- RuntimeConfigResource
- SessionResource
- PlayerRuntimeResource
- MapRuntimeResource
- InventoryResource
- QuestResource
- SkillResource
- BuffResource
- NpcStateResource
- RuntimeQueueResource
- RuntimeClockResource
- ObjectIdAllocatorResource
- Stage5SystemsResource
- GroupResource
- PlayerPermissionResource
- PotionRecoveryResource

迁移顺序：
1. 先把 tick 移到 RuntimeClockResource
2. 再把 object id counters 移到 ObjectIdAllocatorResource
3. 再把 language/account/version 字段移到 SessionResource
4. 再把 inventory/equipment/storage 字段移到 InventoryResource
5. 再把 quest 字段移到 QuestResource
6. 再把 skill 字段移到 SkillResource
7. 再把 buff 字段移到 BuffResource
8. 再把 NPC 字段移到 NpcStateResource
9. 再把 pending actions 移到 RuntimeQueueResource
10. 最后把 map 字段移到 MapRuntimeResource

不要改变行为。这只是结构性重构。
```

---

## 9. 最终目标形态

长期目标不是简单地把 `SimulationResources` 拆成多个 Resource。

真正目标应该是：

```text
Session 只代表连接和身份
MapInstance 持有 ECS World
玩家、怪物、NPC、掉落都是 Entity
状态变化通过 GameEvent 表达
协议包只是 GameEvent 的输出适配
Crystal 只是兼容层，不是核心架构
```

最终形态：

```text
GameServer
  ├── SessionManager
  ├── MapInstanceManager
  ├── PersistenceService
  ├── NetworkGateway
  └── Analytics/Event Pipeline

MapInstance
  ├── ECS World
  ├── AOI Grid
  ├── Collision
  ├── RuntimeClockResource
  ├── RuntimeQueueResource
  └── GameEvent Queue

Session
  ├── connection_id
  ├── account_id
  ├── character_id
  └── controlled_entity
```

---

## 10. 一句话结论

`SimulationResources` 现在能跑，但它承担的职责太多。

短期建议：

```text
不要重写玩法，只做结构拆分。
```

中期建议：

```text
把状态拆成多个小 Resource，并引入 GameEvent。
```

长期建议：

```text
Session 和 MapInstance 分离，World 属于地图实例，玩家只是其中一个 Entity。
```

这样后面再做多人 MMO、反外挂、状态同步、WebSocket、Redpanda、ClickHouse、甚至见证网络，都会轻很多。
