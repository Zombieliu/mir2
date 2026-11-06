# Crystal Ground Item Contract

## 文档状态与证据边界

本文件只记录 Crystal ground item / gold drop、pickup、身份与持久化边界，供
mir2-web3 后续实现和验收使用。它不是“已完成”声明，也不代表 100% parity。
本轮只做源码取证；没有修改代码、测试或其他文档，没有运行 GUI。

权威来源是 git root 下 sibling `Crystal/` 源码，下面的文件路径均相对 git root
书写。Web 行为只能作为回归信息，不能替代下面的 Crystal source fact。

- Crystal 工作树审计时的 `HEAD`：`484983404e3d6afa584e93801f8006ae3429bea9`
- Crystal source worktree 审计时为 dirty：`sourceRootClean=false`。
- source inventory 共发现 `403` 个 `.cs` 文件；source inventory aggregate SHA-256：
  `aad6086d4e0833827571d222b7ca978256210e6dbcbf1300c0decfc6a01cc25e`。
- 因为 `sourceRootClean=false`，本文件的 source facts 绑定上述 aggregate 所代表的
  当时精确工作树内容，而不是仅绑定 Crystal `HEAD`；任何 source 文件变化都必须
  重新生成 aggregate 并使旧事实失效。
- mir2-web3 审计时的 `HEAD`：`bc9ba5055ed08b86bb0d3a01da3df760df01d356`
- mir2-web3 工作树存在其他未提交变化；因此上面的 implementation HEAD 不是本文件
  所称的最终实现 revision。实现验收必须额外绑定实际工作树提交、包身份和证据哈希。
- 标签含义：
  - `VERIFIED_SOURCE_FACT`：可由 Crystal 源码直接定位并复述的事实。
  - `INFERENCE`：由多个 source fact 推导出的验收含义，仍需运行时 trace 或状态
    证据确认。
  - `IMPLEMENTATION_GAP`：当前 mir2-web3 的已知边界或尚未证明的行为；不是说
    该行为永远不能实现。

## Crystal 语义契约

### 1. UserItem 的身份与完整状态

`VERIFIED_SOURCE_FACT` — `Crystal/Shared/Data/ItemData.cs:277-336`,
`UserItem` 定义了一个物品实例的身份和状态。除 `UniqueID`、`ItemIndex`、
`Info`、`CurrentDura`、`MaxDura`、`Count` 外，还包括 `GemCount`、精炼值和精炼
增量、灵魂绑定、`Identified`、`Cursed`、`WeddingRing`、递归 `Slots`、回购/到期
信息、租赁信息、封印信息、商店标志、`Awake`、`AddedStats` 和 `GMMade`。因此
“地面物品”不能用 key/name/quantity 三元组代表。

`VERIFIED_SOURCE_FACT` — `ItemData.cs:331-336`，构造 `new UserItem(info)` 时
`SoulBoundId=-1`、创建空的 `AddedStats` 并按 `Info.Slots` 设置嵌套槽位长度；其余
默认值来自字段初始化。这个默认构造路径与保留原实例是两种不同语义。

`VERIFIED_SOURCE_FACT` — `ItemData.cs:470-524`, `UserItem.Save` 序列化 UID、
耐久、数量、绑定、识别/诅咒位、每个嵌套 slot、宝石数量、附加属性、觉醒、精炼、
婚戒、到期、租赁、商店、封印和 GM 标志。`ItemData.cs:679-724`, `UserItem.Clone`
保留 UID、耐久、数量、宝石、灵魂绑定、识别/诅咒、嵌套 slots、AddedStats、
Awake、租赁/封印等部分字段；它是 outbound packet 的 clone，不应被误认为完整
持久化快照的规范替代品。特别是源码中的 Clone 初始化列表没有复制
`RefinedValue`、`RefineSuccessChance`、`WeddingRing` 等字段；这必须在 packet
对比中单独记录，而不能用“调用了 Clone”推断所有字段均已传递。

### 2. 全局 UID 分配与 CreateFreshItem

`VERIFIED_SOURCE_FACT` — `Crystal/Server/MirEnvir/Envir.cs:4396-4407`,
`Envir.CreateFreshItem(ItemInfo info)` 做四件事：创建新的 `UserItem(info)`；将
`UniqueID` 设为 `++NextUserItemID`；把当前/最大耐久设为 `info.Durability`；调用
`UpdateItemExpiry(item)` 后返回。这里没有复制原 `UserItem` 的随机属性、嵌套 slot
内容、租赁、封印、精炼或灵魂绑定状态。

`VERIFIED_SOURCE_FACT` — `Envir.cs:4413-4429`, `CreateDropItem` 也是使用全局
`NextUserItemID`，但还会按掉落逻辑设置随机当前耐久、执行 `UpgradeItem`、更新
到期，并在不需要鉴定时标记 `Identified=true`。因此“怪物生成掉落的新物品”和
“玩家把现有物品分堆丢下”的新鲜副本不是同一入口。

`VERIFIED_SOURCE_FACT` — `Envir.cs:2584-2594`，账户保存流写入
`NextUserItemID`，说明 UID 计数器本身是账户/服务器持久化状态的一部分，而不是
每个角色或每个地面对象的临时编号。

### 3. PlayerObject.DropItem：full stack、partial stack、DestroyOnDrop

`VERIFIED_SOURCE_FACT` — `Crystal/Server/MirObjects/PlayerObject.cs:7393-7460`,
`PlayerObject.DropItem(ulong id, ushort count, bool isHeroItem)` 先创建失败的
`S.DropItem` 响应；死亡、地图 `NoThrowItem`、找不到指定 UID、数量越界、数量为零、
`DontDrop` 或租赁 `DontDrop` 都会保持失败并 enqueue 失败包。它按
`isHeroItem=false` 查 `Info.Inventory`，按 `isHeroItem=true` 查当前 Hero 的
inventory；hero 不存在时失败。

`VERIFIED_SOURCE_FACT` — `PlayerObject.cs:7462-7478`，当
`temp.Count == count`（full stack）时，若没有 `BindMode.DestroyOnDrop`，把库存中的
同一个 `UserItem temp` 传给 `HumanObject.DropItem`；落地成功后才从原 inventory
清空。若有 `DestroyOnDrop`，跳过地图放置，随后仍清空 inventory 并报告成功；此
路径不产生地面对象。

`VERIFIED_SOURCE_FACT` — `PlayerObject.cs:7480-7490`，当 `count < temp.Count`
（partial stack）时，先执行 `UserItem temp2 = Envir.CreateFreshItem(temp.Info)`，
再设置 `temp2.Count=count`；只有没有 `DestroyOnDrop` 时才把 `temp2` 交给
`HumanObject.DropItem`；最后从原 `temp.Count` 扣减 `count`。

`VERIFIED_SOURCE_FACT` — `PlayerObject.cs:7483-7486` 的时序非常关键：partial
路径在检查 `DestroyOnDrop` 之前已经调用 `CreateFreshItem`。因此 partial +
`DestroyOnDrop` 虽然不生成地面对象，仍消耗一次全局 `NextUserItemID`；full +
`DestroyOnDrop` 不创建 fresh item，不消耗这一次新 UID。这是 UID 流 acceptance leaf，
不能只断言“地面上没有物品”。

`VERIFIED_SOURCE_FACT` — `PlayerObject.cs:7492-7514`，只有对应操作成功后才设置
`p.Success=true` 并 enqueue `S.DropItem`；然后刷新玩家或 Hero 的背包重量并记录
`Report.ItemChanged` / `ItemChangedHero`。full stack 的状态变化是移除原槽位，partial
stack 的状态变化是扣减原槽位；`DestroyOnDrop` 的成功是消耗/移除但没有地面对象。

`VERIFIED_SOURCE_FACT` — `PlayerObject.cs:7393-7395` 和 `Shared/ServerPackets.cs:1667-1690`，
失败和成功的 `S.DropItem` 都携带请求的 `UniqueID`、`Count`、`HeroItem`、`Success`。
这不是地面物品的身份协议；地面广播另见下文。

### 4. HumanObject.DropItem 的放置后 Meat 耐久时序

`VERIFIED_SOURCE_FACT` — `Crystal/Server/MirObjects/HumanObject.cs:1380-1390`,
`HumanObject.DropItem(UserItem item, int range, bool DeathDrop)` 先构造
`ItemObject`，调用 `ob.Drop(range)`；只有 `ob.Drop` 成功后，若 `item.Info.Type ==
ItemType.Meat`，才对传入的同一 `UserItem` 执行 `CurrentDura = max(0,
CurrentDura-2000)`，然后返回成功。

`INFERENCE` — full stack 传入的是库存中的同一个 `UserItem`，所以 Meat 的 ground
object 持有的耐久会在“成功落地后、后续发送/采样前”下降 2000。partial stack 传入
的是刚由 `CreateFreshItem` 创建的 `temp2`，因此 ground copy 的耐久也会发生同样
的 post-placement 变化，但原库存 `temp` 的耐久不会因为这段代码被扣 2000。任何
authoritative ground snapshot 必须在该调用完成后采样，不能在 `ItemObject` 构造
时提前采样。

### 5. ItemObject 的实际持有对象、放置失败和生命周期

`VERIFIED_SOURCE_FACT` — `Crystal/Server/MirObjects/ItemObject.cs:8-45`,
服务端 `ItemObject` 的 `Item` 字段直接引用实际 `UserItem`；gold 使用同一对象的
`Gold` 字段。`ItemObject` 是非阻挡对象。

`VERIFIED_SOURCE_FACT` — `ItemObject.cs:48-116`，普通掉落超时时间是
`Envir.Time + Settings.ItemTimeOut * Settings.Minute`；死亡掉落使用
`Settings.PlayerDiedItemTimeOut * Settings.Minute`；手工构造器同样使用普通
`ItemTimeOut`。这些构造器本身都没有设置 `Owner`。

`VERIFIED_SOURCE_FACT` — `Crystal/Server/MirObjects/ItemObject.cs:190-270`,
`ItemObject.Drop(int distance)` 从距离 0 到指定 distance 搜索合法点：地图存在、
坐标在界内、`CurrentMap.ValidPoint` 为真、不是 movement source；阻挡对象拒绝，
地面 item 数量达到 `Settings.DropStackSize` 的格子拒绝；空格子立即落地，否则选
数量最少的可用格子。没有任何候选格子时返回 false，且不会把 object 加入地图。

`VERIFIED_SOURCE_FACT` — `HumanObject.cs:1380-1390` 与 `PlayerObject.cs:7462-7514`
共同确定 drop failure 的原子边界：`DropItem` 失败时调用方保留原库存，发送失败
`S.DropItem`；成功落地后才允许 full 清槽或 partial 扣量，并最终发送成功包。

### 6. 怪物掉落 owner/group/timeout 与玩家手工掉落的区别

`VERIFIED_SOURCE_FACT` — `Crystal/Server/MirObjects/MonsterObject.cs:1147-1175`,
普通怪物掉落先由 `Envir.CreateDropItem` 创建物品；`DropItem` 构造 `ItemObject`
时设置 `Owner=EXPOwner`、`OwnerTime=Envir.Time + Settings.Minute`，然后尝试
`ob.Drop(Settings.DropRange)`。怪物掉落还受 `CurrentMap.Info.NoDropMonster` 拒绝。

`VERIFIED_SOURCE_FACT` — `MonsterObject.cs:1177-1189`，普通怪物 gold 也设置
`Owner=EXPOwner` 和一分钟 `OwnerTime` 后落地；龙掉落使用
`Crystal/Server/MirEnvir/Dragon.cs:180-208` 的同样一分钟 owner 窗口。

`VERIFIED_SOURCE_FACT` — `Crystal/Server/MirObjects/PlayerObject.cs:7530-7570`,
`PlayerObject.PickUp()` 只检查当前地图当前格子的 item object。若 `ob.Owner` 非空、
且不是当前玩家、且当前玩家不是 owner 的 group member，就记为 owner blocked；遍历
其他候选后发送 `CannotPickupNotOwner` system chat。玩家手工 drop 的 ItemObject
构造器没有设置 owner，不能把 monster-drop 的 owner 一分钟规则自动套到手工
player drop。

`VERIFIED_SOURCE_FACT` — `ItemObject.cs:125-142`，处理地面对象时先检查
`Envir.Time > ExpireTime`，到期则 `CurrentMap.RemoveObject(this)` 后 `Despawn()`；
否则 owner 超过 `OwnerTime` 时被清为 null。`ItemObject.cs:144-164` 将 owner timeout
和 expire timeout 纳入下一次处理时间。

### 7. Pickup admission、GainItem/Gold、移除和成功包顺序

`VERIFIED_SOURCE_FACT` — `PlayerObject.cs:7530-7570`：

1. 过滤当前 cell 中的 `ObjectType.Item`。
2. 先执行 owner/group admission。
3. 对 `item.Item` 先执行 `CanGainItem(item.Item)`；失败时保留地面对象并继续找
   其他候选。
4. 成功时可先向 group members 发 `PickedUpItem` system chat（只有
   `ShowGroupPickup` 且 picker 是 group member）。
5. 调用 `GainItem(item.Item)`。
6. 调用 `Report.ItemChanged(item.Item, item.Item.Count, 2)`。
7. 调用 `CurrentMap.RemoveObject(ob)`，随后 `ob.Despawn()`。
8. 返回，不再继续同一格的其他物品。

gold 分支在 `PlayerObject.cs:7560-7565` 先 `CanGainGold`，再 `GainGold`，再
`CurrentMap.RemoveObject` 和 `ob.Despawn`。失败不移除地面对象。

`VERIFIED_SOURCE_FACT` — `Crystal/Server/MirObjects/HumanObject.cs:7586-7625`,
`CanGainItem` 在有空位时允许；没有空位时只对可堆叠类型检查同类堆叠容量。对普通
非堆叠 item，满背包即拒绝。

`VERIFIED_SOURCE_FACT` — `HumanObject.cs:7829-7833`，`GainItem` 先 `CheckItem(item)`，
再建立 outbound `item.Clone()` 并 enqueue `S.GainedItem`，随后把原 `item` 传给
`AddItem(item)` 并刷新背包重量。`HumanObject.cs:1577-1629` 的 `AddItem` 对
`StackSize > 1` 先按同一 `ItemInfo` 合并；只有无法完全并入既有 stack 时，才把传入
item 的余量放入 belt 或 inventory 空槽。因此不能笼统声称所有 full stack pickup
都会在背包保留 ground UID：

- non-stackable，或没有可合并既有 stack：原 `UserItem` 引用进入空槽，UID 可保留；
- stackable 且整组完全并入既有同 `ItemInfo` stack：权威背包中不会新增/保留该
  ground item UID，但 `GainedItem` outbound packet 仍来自拾取前的 `Clone()`；
- stackable 且只部分并入：既有 stack 增加数量，剩余数量的原 item 引用进入空槽，
  所以只保留余量对应的 UID 记录。

这是 Crystal 的“packet clone 与 authoritative inventory merge 分离”语义，字段规则
必须分别验收。

`VERIFIED_SOURCE_FACT` — `Crystal/Server/MirObjects/MapObject.cs:358-378`,
`Despawn()` 先广播 `S.ObjectRemove { ObjectID }`，再从全局对象表移除、清理 action
并把 `Node=null`。因此成功 pickup 的地面移除 packet 由 despawn 产生，且发生在
`GainItem`/`GainGold` 之后。

`VERIFIED_SOURCE_FACT` — `Crystal/Server/MirObjects/ItemObject.cs:365-383`
和 `Crystal/Shared/ServerPackets.cs:2128-2255`：地面 `ObjectItem` 只含
`ObjectID`、显示名称/颜色、位置、图像和 grade；不携带 `UserItem.UniqueID` 或
完整嵌套元数据。`GainedItem` 才通过 `UserItem.Save` 发送 item；`GainedGold` 只
携带数量。地面客户端显示不能作为完整物品状态证据。

### 8. 重复拾取、并发和原始源码无法证明的边界

`VERIFIED_SOURCE_FACT` — 在单次 Crystal `PickUp()` 调用内，成功路径在
`Despawn()` 后立即 return；同一个已脱离地图的 object 不会被该调用再次处理。

`INFERENCE` — 这足以定义“同一串行处理上下文内 exactly once”的验收，但这些源码
片段没有提供跨线程重复 client packet、重复 socket、网关重放或崩溃恢复的独立
幂等键/事务日志证明。不能仅凭 `Node=null` 或 `ObjectRemove` 声称分布式 exactly
once。

`INFERENCE` — 多个玩家同时拾取同一 object 的最终结果必须观察 Crystal 的实际
网络处理/线程调度；当前源片段可证明 admission、Gain、Remove 的顺序，不能单独
证明跨连接竞态下谁先赢或是否存在外部锁。

### 9. 断线、保存与重新登录边界

`VERIFIED_SOURCE_FACT` — `Crystal/Server/MirObjects/PlayerObject.cs:266-365`,
`StopGame` 在玩家仍有节点时记录当前有效地图/坐标，并处理宠物、英雄、魔法和 buff
等退出状态；`PlayerObject.cs:389-430` 随后移除玩家、从地图移除并广播玩家的
`ObjectRemove`，清理 group/trade/rental 等关系，写 LastIP/LastLogoutDate，
记录 disconnect 并 cleanup。这里没有把地面 ItemObject 序列化进角色 `Info`。

`VERIFIED_SOURCE_FACT` — `Crystal/Server/MirNetwork/MirConnection.cs:1126-1145`,
正常 `LogOut()` 在 Game stage 调用 `Player.StopGame(23)`，切回 Select，置空 Player，
发送 `S.LogOutSuccess`；`MirConnection.cs:760-829` 的 soft/hard disconnect 也会在
清理连接时调用 `Player.StopGame(reason)`。

`VERIFIED_SOURCE_FACT` — `Crystal/Server/MirDatabase/CharacterInfo.cs:391-455`
及其后续 Save 字段，角色保存包含地图/坐标/方向、HP/MP/经验、inventory、equipment、
quest inventory 等 `UserItem.Save` 数据。地面 ItemObject 不在 CharacterInfo 的
角色 item 数组中。

`VERIFIED_SOURCE_FACT` — `Crystal/Server/MirEnvir/Envir.cs:2153-2160`，
环境按 SaveDelay 周期调用 `BeginSaveAccounts`；`Envir.cs:2570-2582` 的同步保存先
写 `AccountPath+n`，再轮换旧文件并改名；`Envir.cs:2776-2818` 的异步保存路径会
准备备份、写临时文件并在回调中轮换文件。异常路径写入 `MessageQueue` 或被捕获。

`INFERENCE` — Crystal 源码能证明“退出会更新内存角色状态，周期/关闭流程会保存
角色”，但不能仅由这些片段证明某个具体 ground item 在断线瞬间一定已经写入磁盘，
也不能证明保存失败后的重试、journal 恢复、跨进程原子性或宕机恢复顺序。必须用
实际保存/重载 trace 和注入的保存失败负例补齐。

## mir2-web3 当前投影与有损边界

以下是对当前工作树的只读对照，不是实现通过声明。

### `apps/simulation/src/config.rs`

`IMPLEMENTATION_GAP` — `apps/simulation/src/config.rs:4355-4405` 的
`WorldItemSnapshot` 只承载 key/name/icon/unique_id/slot/container/quantity/
description/两项 durability/sell value/equip slot/grade/added attack/defence。
相对于 Crystal `UserItem`，未承载或未以同等结构承载 `ItemIndex`、`GemCount`、
`RefinedValue`、`RefineAdded`、`RefineSuccessChance`、`DuraChanged`、`SoulBoundId`、
`Identified`、`Cursed`、`WeddingRing`、递归 `Slots`、`BuybackExpiryDate`、
`ExpireInfo`、`RentalInformation`、`SealedInfo`、`IsShopItem`、完整 `Awake`、
完整 `AddedStats` 和 `GMMade`。

`IMPLEMENTATION_GAP` — `config.rs:4407-4450` 的 `GroundDropSnapshot` 只有
object/display/location/source/owner deadline 与数量；`GroundDropLootSnapshot::InventoryItem`
只有 key、显示字段、durability、两项 added attack/defence、有限 `added_stats`、
cursed、socket_slots、show_group_pickup。它没有 ground `unique_id`，没有完整
`UserItem` 树，也没有把 full-vs-partial 的身份来源、UID 流或 fresh/default marker
编码进 contract。

### `apps/simulation/src/runtime/drops.rs`

`IMPLEMENTATION_GAP` — `drops.rs:172-219` 的 `DropLoot::InventoryItem` 与
`ResolvedDropTemplate` 是有限字段模型；`drops.rs:2388-2500` 的 player drop 由
inventory `ItemState` 复制这些有限字段，partial 与 full 都走同一个有限 payload
形状。当前代码没有让 full stack 保留 Crystal 原 `UserItem` 的全部嵌套状态，也没有
让 partial 明确记录“fresh item 已经消耗全局 UID”的事实。

`IMPLEMENTATION_GAP` — `drops.rs:2388-2500` 直接 `if hero_inventory { return
failed_packet; }`，而 Crystal `PlayerObject.DropItem` 明确支持 hero inventory
（`PlayerObject.cs:7410-7438` 查当前 Hero inventory，`7462-7490` 继续执行 full/
partial 语义）。这项 gap 不应被当前竖切范围掩盖成全部 parity。

`IMPLEMENTATION_GAP` — `drops.rs:2278-2335` 的 personal pickup 与
`drops.rs:3142-3260` 的 shared pickup 使用 `add_or_increment_item_with_*`，而非
将 authoritative ground item 原实例完整转移回 inventory；`runtime/inventory.rs:1640-1795`
构造新 `ItemState`，调用 `allocate_item_unique_id`，并按有限字段推导 durability、
grade、attack/defence、socket 等。当前实现因此既不能证明 Crystal full-stack 在
non-stackable/无既有 stack 前提下保留 UID+全部嵌套元数据，也不能证明 stack merge
时 outbound clone、完全合并导致 UID 消失、部分合并保留余量 UID 的三种结果完全
一致。

`IMPLEMENTATION_GAP` — `drops.rs:2030-2040` 的 owner expiration 使用 tick 比较，
`drops.rs:1988-2017` 允许 owner、group member 或超时后任意 picker；当前模型只在
drop payload 上放 `DropOwnership`，未证明与 Crystal 的 ItemObject owner 在玩家
手工 drop、怪物 drop、龙 drop、死亡 drop 四类入口完全区分。

`IMPLEMENTATION_GAP` — `drops.rs:2140-2190` 的 ground placement 已有合法点、阻挡、
movement source 和 stack cap 检查，结构上接近 Crystal；但当前 acceptance 仍需
验证所有失败路径不改变 inventory/ground authoritative state，并且与 Crystal 的
逐格搜索顺序、距离、边界一致。

### `apps/gateway/src/routing.rs`

`IMPLEMENTATION_GAP` — `routing.rs:2435-2455` 保存 shared monster kill award 的
`GroundDropSnapshot`，并注释说明 `ObjectItem/ObjectGold` 只含 legacy client-facing
字段；这证明 shared map 中的权威 snapshot 与广播包已经被概念上区分，但 snapshot
本身仍是上面的有限结构。

`IMPLEMENTATION_GAP` — `routing.rs:5315-5375` 的
`ground_drop_snapshot_from_spawn_packet` 从 `ObjectItemInfo` 重建 snapshot 时把
key/name 当作 key、quantity 固定为 1、description/weight/durability/added stats/
cursed/socket/group 设为默认值，也没有 UID。这是从 legacy spawn packet 回填 shared
state 的明确 lossy boundary；不能在已丢失后声称恢复了 Crystal UserItem。

`IMPLEMENTATION_GAP` — `routing.rs:7540-7605` 的 local ground drop remap 只把
session-local object id 映射为 zone object id，并根据显示投影做去重；它没有把
authoritative local `UserItem` 树或 global UID 一起提升到 Zone snapshot。

`IMPLEMENTATION_GAP` — `routing.rs:1680-1745` 的 `ZoneMapSnapshotLayer` 会持久化
`ground_drops`、removed ids 和 owner/expiry deadlines，但字段是否足够复原 Crystal
的 full/partial item 身份仍是否定的：当前 `GroundDropSnapshot` 本身没有完整 item
树和 UID 流信息。

`IMPLEMENTATION_GAP` — `routing.rs:1126-1305` 将 ground pickup 通过 shared account/
inventory transaction 及业务 key 组织；这可提供 request ordering/idempotency 的
测试入口，但不能替代对同一 ground object 的 authoritative claim、inventory commit、
ObjectRemove 广播、保存和重载的全链路证明。

### `apps/gateway/src/gateway.rs` / Gateway boundary

`IMPLEMENTATION_GAP` — Gateway 的 shared packet bridge 只能从 `ObjectItem`/`ObjectGold`
得到 legacy display projection；Crystal 的这些包本来就不携带 UID（`ServerPackets.cs:2128-2255`），
所以 Gateway 必须在 authoritative simulation/zone event 处取得完整 item snapshot，
不能依赖客户端 spawn packet 反推。若 bridge 只接受 packet，full-stack metadata
已经不可逆丢失。

`INFERENCE` — 当前 shared transaction receipts 能记录 committed/packets，但没有
从 Crystal 源码可直接证明的“地面对象 claim + inventory mutation + persistence
commit”单一原子边界。这个边界必须通过 authoritative state、packet sequence、
reload state 和保存失败负例一起验收。

## 最小 acceptance matrix

每一项都必须同时提供：

1. authoritative state before/after（包括 ground object、inventory/equipment、UID
   allocator 和 owner/expiry）；
2. owner/AOI/global packet sequence（成功和失败两边都记录，不能只看最后一个包）；
3. persistence evidence（保存前、保存结果、重新登录后的状态）；
4. negative evidence（注入失败或边界条件时，证明没有错误扣除、重复奖励、错误
   despawn、错误包或静默 fallback）。

| Leaf | Crystal contract to exercise | Required evidence / negative case | Status |
|---|---|---|---|
| `DROP.FULL_STACK.UID_AND_TREE_ROUNDTRIP` | full stack 把原 `UserItem` 传入 `HumanObject.DropItem`；仅在 non-stackable 或无既有可合并 stack 前提下，成功拾回后 ground 与 inventory 保留同一 UID 和完整嵌套/附加元数据 | 先证明没有可合并的同 `ItemInfo` stack；ground authoritative snapshot 在 `HumanObject.DropItem` 返回后采样；检查 UID、Slots、AddedStats、Awake、refine、bind/rental/sealed 等；拾回后比较保存/重载；地面 spawn 包缺 UID 不能作为通过证据 | `BLOCKED_EXTERNAL` / `IMPLEMENTATION_GAP` |
| `PICKUP.STACK_MERGE.CLONE_AND_UID_OUTCOME` | `GainItem` 先以拾取前 ground `UserItem` 的 Clone 发 `GainedItem`，再由 `AddItem` 按 `StackSize>1` 和同一 `ItemInfo` 合并 | 三个子例：整组完全并入既有 stack（权威背包中 ground UID 消失）；部分并入且余量入空槽（余量保留原 UID）；无既有 stack（原 UID 入空槽）。每例都比较 outbound clone、最终背包、ObjectRemove、保存/重载和失败负例 | `BLOCKED_EXTERNAL` / `IMPLEMENTATION_GAP` |
| `DROP.PARTIAL.FRESH_UID_DEFAULTS` | `PlayerObject.cs:7480-7490` 先 `CreateFreshItem`，新 UID、fresh defaults、expiry；原堆栈只扣量 | 证明 ground copy 是新 UID；证明 fresh/default/expiry 而非复制原随机状态；证明原 stack 保留原 UID/metadata；拾回与 reload 各自正确 | `BLOCKED_EXTERNAL` / `IMPLEMENTATION_GAP` |
| `DROP.PARTIAL.DESTROY_ON_DROP_UID_FLOW` | `7483` fresh UID allocation 先于 `7485` DestroyOnDrop check；partial DestroyOnDrop 消耗全局 UID、无 ground object；full DestroyOnDrop 不分配 fresh UID | 记录 `NextUserItemID` before/after、inventory、ground、success packet；对 full/partial 两个 case 做 negative comparison | `BLOCKED_EXTERNAL` / `IMPLEMENTATION_GAP` |
| `DROP.MEAT.POST_PLACEMENT_DURA` | `HumanObject.cs:1380-1390` 仅在 `ob.Drop` 成功后对传入同一 UserItem 扣 2000 | 分别测试 full/partial Meat；在构造、落地成功返回、packet、pickup 四个时点采样 CurrentDura；放置失败不得扣耐久 | `BLOCKED_EXTERNAL` |
| `DROP.HERO_INVENTORY.SAME_CONTRACT` | Crystal 支持 `isHeroItem=true`，从当前 Hero inventory 查找并执行 full/partial/DestroyOnDrop | hero full、partial、无 hero、错误 UID、DontDrop、地图禁止 drop；检查 HeroItem 字段和成功/失败包；mir2-web3 当前 hero 直接失败必须保持负例 | `IMPLEMENTATION_GAP` |
| `DROP.PLAYER_MANUAL.NO_IMPLICIT_OWNER` | 玩家手工 ItemObject 构造器设置 expiry，但没有 Owner/OwnerTime | 手工 player drop 后 owner 为 null；另一组 monster drop 才有 owner+一分钟窗口；不能用 group member 代替 owner 事实 | `BLOCKED_EXTERNAL` |
| `DROP.MONSTER_OWNER_GROUP_TIMEOUT` | MonsterObject/Dragon 设置 `Owner=EXPOwner`、`OwnerTime=now+minute`；PickUp 允许 owner/group，超时清 owner | owner、同组、非组未超时、非组超时、owner disconnect、deadline 边界；记录 system chat 和 ground survival | `BLOCKED_EXTERNAL` |
| `DROP.PLACEMENT_FAILURE.ATOMIC` | Crystal `ItemObject.Drop` 无合法点时 false；PlayerObject 失败包且原 inventory 不变 | 填满距离范围/阻挡/transfer-source/stack-cap；证明无 ground object、无 inventory 扣除、无 success packet、无错误耐久变化 | `BLOCKED_EXTERNAL` |
| `PICKUP.CAPACITY_FAILURE.KEEP_GROUND` | `CanGainItem` 先于 `GainItem`；背包满且无可堆叠容量则不移除 ground | 记录失败 system message（如适用）、无 GainedItem、无 ObjectRemove、ground snapshot 与 inventory 不变；保存/重载后 ground 仍存在或按明确世界持久化规则存在 | `BLOCKED_EXTERNAL` |
| `PICKUP.SUCCESS.ORDER` | Item: CanGain → optional group chat → GainItem/Report → Map.RemoveObject → Despawn/ObjectRemove；gold 类似但发送 GainedGold | 精确 packet sequence 与 authoritative mutation timeline；验证 ObjectRemove 不早于 inventory commit；持久化后 reload 只出现一次 item/gold | `BLOCKED_EXTERNAL` |
| `PICKUP.DUPLICATE.EXACTLY_ONCE` | 单次串行 Crystal PickUp 成功后 despawn 并 return；跨连接竞态不能由本源码片段直接证明 | 重复相同 request、重复 object id、双 picker 同时请求、断线重放；必须证明最多一次 Gain、最多一次 ObjectRemove、无双份持久化奖励 | `BLOCKED_EXTERNAL` |
| `DROP.DISCONNECT.RELOGIN` | Player StopGame 清理 player/map presence；角色 inventory/position 由 CharacterInfo 保存，地面对象不在角色数组 | drop→disconnect/timeout→save→relogin；分别验 ground owned by shared Zone 与 private player item；要求 ground、inventory、UID allocator 一致 | `BLOCKED_EXTERNAL` |
| `SAVE.FAILURE.ORDERING` | Crystal 周期/关闭保存写临时文件并轮换；源码未证明 ground journal 与角色保存的分布式原子性 | 注入角色保存失败、checkpoint 写失败、journal 写失败、进程重启；负例必须证明不会先发“成功/已移除”而后丢 item，也不得静默清理未持久化状态 | `BLOCKED_EXTERNAL` |
| `DROP.UID.PERSISTENCE.GLOBAL_COUNTER` | `Envir.NextUserItemID` 在账户保存中持久化；fresh/drop item 分配都递增 | full 不递增 fresh UID；partial 递增；partial DestroyOnDrop 也递增；重启/重载后不得复用旧 UID；跨角色/跨 zone collision 必须失败闭合 | `BLOCKED_EXTERNAL` / `IMPLEMENTATION_GAP` |

## 当前不能从 Crystal 源码单独证明的边界

- `PlayerObject.PickUp` 的源码给出串行函数顺序，但没有单独给出多个 socket/线程对同一
  ground object 的分布式幂等协议；需要运行时竞争 trace 或更高层连接调度证据。
- `StopGame`、`CharacterInfo.Save` 和 `Envir.BeginSaveAccounts/EndSaveAccounts` 能证明
  内存退出处理与账户文件保存路径，但不能证明 ground item journal、Gateway shared
  Zone checkpoint、角色文件和 packet acknowledgement 的跨系统原子事务。
- 源码没有为本项目定义 mir2-web3 的 authoritative Zone snapshot schema；因此不能
  直接从 Crystal 推出 web/native bridge 应该采用哪一种 UID namespace、冲突处理或
  journal 格式。
- `ObjectItem` 地面广播刻意不含完整 `UserItem`；仅凭客户端截图、ObjectItem 包或
  Web UI 状态无法证明 UID、嵌套槽位、租赁/封印和保存正确。
- 具体 `Settings.ItemTimeOut`、`PlayerDiedItemTimeOut`、`DropRange`、`DropStackSize`
  数值以及 map `NoThrowItem`/`NoDropMonster` 数据值，需要绑定同一 Crystal 配置和
  map/item data revision；本文件只证明读取这些配置的 symbol 与相对时序。
- 当前 mir2-web3 的 hero inventory、full/partial item tree、global UID counter、
  player-vs-monster owner distinction、Meat post-placement durability、save/journal
  failure ordering 均未因本文件而变成 VERIFIED。

## 后续实现/验收约束

1. 先定义一个不可丢字段的 authoritative ground item record；legacy `ObjectItem`
   只能作为显示 projection，不能反向重建该 record。
2. full stack 记录原 `UserItem` 身份；partial stack 明确调用 fresh-item allocator，
   包含 DestroyOnDrop 仍消耗 UID 的路径；两者不能共享一个“复制有限字段”的快捷
   helper。
3. player manual drop、monster/dragon drop、death drop、hero drop 必须有独立 owner、
   timeout 和 inventory source 测试，不以当前竖切裁剪掉 hero 语义。
4. 地面放置成功后再采样 Meat durability；放置失败时原 item 和 durability 必须保持
   不变。
5. pickup 必须先完成 authoritative claim/capacity admission，再产生 Gain 和
   ObjectRemove；任何 persistence/journal 失败都应保留可恢复状态，不能先清理地面
   对象再把失败隐藏为成功。
6. 最终 ledger leaf 还必须同时绑定 Crystal trace、mir2-web3 trace、normalized
   diff、negative evidence、reload evidence，以及对应 implementation revision/package
   identity；本文件本身不是这些证据的替代品。
