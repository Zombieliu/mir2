# ElectricShock（诱惑之光）完整归属缺口诊断

## 审查范围

- 原版：`E:/mir2/Crystal`
- R4 冻结源码：`C:/mir2-skill-page-candidate-20260910-snapshot/mir2-web3`
- R4 冻结 HEAD：`e7fd008b287607f15e2ab6f67519d77ea71ebeb2`
- 当前源码：`E:/mir2-player-journey/mir2-web3`
- 当前 HEAD：`41a3101924aabd63f000f11c155191138e982dc7`

本次仅做源码读取与比较，没有修改或构建上述两个项目，也没有操作 UI。当前仓库整体很脏，因此没有用它替代 R4 行为。对 ElectricShock 直接相关的 `zone/runtime.rs`、`zone/types.rs`、`zone/packets.rs`、`zone/runtime/checkpoint.rs` 和 `gateway/routing.rs` 做了 `git diff --no-index`；除行尾提示外没有语义差异。`game-data/src/lib.rs` 与生成的怪物清单在两份源码中 SHA-256 相同。因此下述 R4 缺口在当前源码相关路径中仍存在。

## 确切结论

R4 共享 Zone 的 ElectricShock 只实现了“冻结普通敌对怪物 `10 + 5 × 技能等级` 秒”。它没有实现原版技能的诱惑归属状态机：

- 没有原版的第一段技能等级命中概率。
- 没有命中后的 50% 震慑 / 50% 继续诱惑分支。
- 没有玩家等级、`CanTame`、Boss 上限和宠物容量检查。
- 没有按怪物最大 HP 决定最终诱惑成功率。
- 没有从旧 Master 转移到新 Master。
- 没有设置 owner session、Master object id、宠物最大等级和一小时驯服期限。
- 没有从原生 respawn 槽摘除已诱惑怪物。
- 没有广播原版的震慑/狂暴颜色、归属名称和转主后的血量。
- 没有角色宠物保存、跨地图转移和重登录恢复。
- 共享 Zone 施法成功只同步扣 MP 与冷却，没有执行原版条件式 `LevelMagic`，所以技能熟练度也缺失。

而且当前“冻结”并非严格的原版震慑：R4 对每个通过通用目标检查的 ElectricShock 都无条件设置控制时间，立即生效；原版在施法 500ms 后才解析，并且大量施法会完全无效果、只狂暴或才进入诱惑尝试。

## 原版规则

### 施法与延迟

`E:/mir2/Crystal/Server/MirObjects/HumanObject.cs:3400-3452` 先验证技能、范围、冷却和 MP，然后立即扣 MP。`3519-3521` 对 ElectricShock 安排 `Envir.Time + 500ms` 的 `DelayedType.Magic`。`6031-6037` 在延迟执行时再次确认目标仍是同图、有效、可攻击的 `MonsterObject`，再调用 `ElectricShock`。

这意味着扣 MP、起手动画与 500ms 后的技能结果是两段生命周期。目标在这 500ms 内失效时，不产生震慑或诱惑。

### 完整概率树

原版核心在 `HumanObject.cs:4009-4102`。

第一段等级门：

```text
Random.Next(4 - magic.Level) == 0
```

技能等级 0/1/2/3 的通过率分别为 `1/4`、`1/3`、`1/2`、`1`。未通过时有 `1/2` 机会调用一次 `LevelMagic`，然后结束；通过时必定调用一次 `LevelMagic`。

通过第一段后：

1. 目标已由当前施法者控制：只刷新 `ShockTime = now + (level*5+10)s`，清空目标，不重新诱惑。
2. 否则再做 `Random.Next(2)`：一半只震慑相同时长并结束；另一半继续诱惑。
3. 诱惑资格：目标等级不得高于玩家等级 `+2`，且 `MonsterInfo.CanTame=true`。
4. Boss 还受 `Settings.MaxBossTames` 限制；原版默认值为 1，见 `Server/Settings.cs:151-153,433-434`。
5. 难度门：`Random.Next(player.Level + 20 + magic.Level*5) > target.Level + 10` 才继续。未通过时，若目标当前没有 Master，`4/5` 概率进入 10–29 秒 Rage 并清空目标。
6. 容量门：存活且非 intelligent creature 的普通宠物数量必须小于 `magic.Level + (Globals.MaxPets - 3)`。`Shared/Globals.cs:23` 的 `MaxPets=5`，所以 0/1/2/3 级技能上限为 2/3/4/5 只。
7. 最终 HP 门使用怪物**最大 HP**：`rate=max(2, 2*(MaxHP/100))`（当 `MaxHP/100 <= 2` 时取 2），仅当 `Random.Next(rate)==0` 时成功。因此最终概率是 `1/rate`，高血量怪物明显更难诱惑。

不含资格、难度和 HP 门时，进入诱惑分支的基础概率为：0级 `1/8`、1级 `1/6`、2级 `1/4`、3级 `1/2`。这不是“技能等级越高就直接成功”。

### 成功后的归属与等级

`HumanObject.cs:4073-4102`：

- 若目标已有旧 Master，把目标 HP 设为最大 HP 的 10%，并从旧 Master 的 `Pets` 移除。
- 若目标是野生 respawn 怪物，递减 respawn/map/environment 计数并将 `target.Respawn=null`；它不再作为原出生点的普通野怪复活。
- 设置 `target.Master=this` 并加入新 Master 的 `Pets`。
- 清空 Target、RageTime、ShockTime、OperateTime。
- 设置 `MaxPetLevel = 1 + magic.Level*2`，即技能 0/1/2/3 对应宠物最高 1/3/5/7 级。当前宠物等级独立，野怪被诱惑时通常从 0 开始。
- `Settings.PetSave=false`（默认）时设置一小时 `TameTime`。
- 广播血量变化和 `ObjectName`。怪物 `Name` 属性在有 Master 时为 `怪物名(Master名)`，见 `MonsterObject.cs:508-513`。

宠物经验不是技能等级的别名。`MonsterObject.cs:3502-3518` 按 `(PetLevel+1)*20000` 经验升级，达到 `MaxPetLevel` 后停止；`811-833` 每级增加 HP 20、AC/MAC 各 2、DC 各 1。玩家获得经验时，同图、可视范围内的存活宠物获得同量宠物经验，见 `PlayerObject.cs:891-900`。

### 震慑、狂暴和广播

原版 `MonsterObject.cs:851-901` 在每次处理时按状态刷新名字颜色：Shock 为 Peru，Rage 为 Red，Hallucination 为 MediumOrchid，变化时广播 `ObjectColourChanged`。`GetInfo` 在 `2857-2879` 为新进入 AOI 的客户端携带剩余 `ShockTime` 和 `MasterObjectId`。诱惑成功另广播血量与 `ObjectName`。

Rage 不只是颜色。`MonsterObject.cs:2522-2539` 允许处于 Rage 的野怪把其他怪物作为攻击目标。因此仅增加红色表现仍不构成原版失败分支。

### 原版离线和期限

`PlayerObject.cs:266-358` 在退出时解除现场 Master、移除现场对象，并把可保存宠物转入角色 `Info.Pets`。默认 `PetSave=false` 时，Wizard 的普通诱惑宠物保存剩余 `TameTime`。`1231-1290` 在再次进入游戏时恢复宠物等级、经验、最大宠物等级、Master、HP 与剩余期限并重新生成。

`MonsterObject.cs:1218-1223` 在 TameTime 到期时从 Master.Pets 移除、解除 Master 并广播恢复后的名称。

需要记录一个原版自身的数据限制：`CharacterInfo.PetInfo` 的运行时结构有 `TameTime`，但 `CharacterInfo.cs:629-655` 所示二进制读写字段只包含 monster index、HP、经验、等级和最高等级，没有写入 TameTime。它能支持同一服务进程内的退出/重登恢复，但服务进程重启后的剩余期限持久性并不完整。mir2-web3 不应复制这个可能导致状态丢失的存储缺陷。

## R4 冻结源码的实际行为

### 共享 Zone 路径

Gateway 对共享怪物目标把客户端 Magic 转为 `ZoneCommand::PlayerCastMagicWithItem`，技能等级、MP 消耗与冷却从个人 session 的权威技能状态取得，见 `apps/gateway/src/routing.rs:10316-10387,10518-10530,10705-10730`。命令随后只走 Zone，不再执行个人 session 的原技能处理，见 `routing.rs:13127-13136`。

`apps/simulation/src/runtime/zone/runtime.rs:4436-4446` 在施法当次立即调用 `apply_native_monster_magic_control`。`5760-5934` 的通用控制路径只检查怪物可见、存活、`hostile_to_player=true`，以及少量通用 AI/poison 限制；随后直接设置 `control_until_ms` 和 AI/攻击 ready time。

`runtime.rs:14189-14201` 对 ElectricShock 固定返回：

```text
duration = (level*5 + 10) seconds
poison = none
effect = none
blocks_ai = true
```

因此 R4 的实际结果是：每次被接受的施法都立即冻结普通敌对怪物 10/15/20/25 秒。没有 500ms 延迟，也没有任何诱惑结果。

`runtime.rs:10272-10286` 在控制有效期内阻止怪物 AI，说明冻结本身会实际生效。`8408-8418` 到期会清 `control_until_ms`；ElectricShock 没有 poison，所以没有状态清除包。

### 单 session 兼容代码不是 R4 共享行为

`apps/simulation/src/runtime/skills.rs:3878-3911` 还有一份旧的个人世界 ElectricShock：它先拒绝 `monster.level > player.level+2`，再把 tracking/移动/攻击推迟同样时长。它同样不诱惑，而且把原版只用于诱惑资格的 `level+2` 错用于震慑。

R4 Gateway 对共享怪物施法走 Zone 路径，不能用这份个人世界实现声称 R4 已有等级规则。

### 技能熟练度没有推进

共享 Zone 接受施法后，Gateway 在 `routing.rs:10786-10790` 只调用 `apply_zone_player_magic_spend`。`apps/simulation/src/runtime/session.rs:1006-1039` 仅同步 MP、冷却和 runtime tick，不调用 `advance_magic_progression`。原版 ElectricShock 的 `LevelMagic` 是概率树的一部分，而 R4 共享路径没有携带“本次应推进熟练度”的 Zone 结果。

### 数据已有，但逻辑没有使用

`packages/game-data/src/lib.rs:1867-1902` 的 `CrystalMonsterTemplate` 已包含 `level`、`hp`、`can_tame` 和 `is_boss`。生成怪物清单也含准确的 `can_tame`、`is_boss`。实现诱惑资格不需要再猜怪物白名单。

R4 没有导入 Crystal 地图的 `NoPets` 标志，`ZoneMapMetadata`（`zone/types.rs:48-59`）也没有该字段，所以原版 `HumanObject.cs:4011-4015` 的禁宠地图拒绝与系统提示目前无数据入口。

## 分层缺口

| 层 | 已有 | 缺失或错误 |
|---|---|---|
| Gateway 入站 | 验证当前玩家 actor；从 session 取权威技能等级、MP、冷却 | 不需要信任客户端等级，但没有接收 Zone 的条件式熟练度结果 |
| Zone 施法 | 检查目标/范围、扣 MP、广播 Magic/ObjectMagic | ElectricShock 结果立即解析，缺 500ms 延迟和重验 |
| 概率 | 有服务端 deterministic roll 工具 | 完整六段概率/资格树全部缺失；当前等价于控制成功率 100% |
| 怪物资料 | 清单有 level/max HP/can_tame/is_boss | 未使用这些字段；无 MaxBossTames 配置；无 NoPets 地图字段 |
| Master | `ZoneNativeMonster` 已有 `owner_session_id`、`master_object_id`、`owner_player_object_id` | ElectricShock 从不写入；无旧主转移事务；无稳定角色所有者标识 |
| 宠物等级 | 现有 summoned monster 有 `summon_skill_level` | 缺独立 pet level/experience/max pet level；不能把召唤技能等级复用为宠物等级 |
| AI | 已有 owner summon 跟随/攻击路径 | 被诱惑怪不会进入该路径；无普通 Rage 状态与怪打怪行为；宠物模式没有完整投影到普通 Zone 宠物 |
| 广播/AOI | ObjectMonster 协议已有 `shock_time`、`master_object_id`；有 ObjectName/Colour/Health 包 | ElectricShock 不更新 retained ObjectMonster，也不发震慑颜色、狂暴颜色、名称、Master 或转主血量 |
| Respawn | Zone 有权威 `native_monster_respawns` | 诱惑成功不摘除 respawn policy；若只改 Master，宠物死亡后仍可能按野怪槽复活 |
| Zone checkpoint | checkpoint 序列化 `objects`、`native_monsters`、respawn policy 和 pending action 容器 | 当前无诱惑状态；恢复时 `checkpoint.rs:746-751` 从原 disposition 重算 hostile，若只改 `hostile_to_player=false`，恢复后会再次变敌对 |
| Leave/换图 | `leave` 调用 owner generated object 清理 | 清理依据 retained `ObjectMonster.master_object_id`（`packets.rs:1238-1250`）；只改 native monster 会泄漏宠物。当前没有输出宠物快照给 session |
| 角色存档 | `CharacterSaveRecord` 有完整私有角色 CAS 快照 | `config.rs:1647+` 没有普通宠物字段；save/session/join 都不能恢复诱惑宠物 |

## 最小安全实现方案

### 1. 把 ElectricShock 作为独立 Zone 状态机

新增 `apps/simulation/src/runtime/zone/runtime/electric_shock.rs`，只在高冲突的 `runtime.rs` 增加模块声明和两个窄调用点：施法时排入 `PendingElectricShock`，tick 时解析。不要继续把诱惑塞进通用 `ZoneNativeMagicControl`。

`PendingElectricShock` 至少保存：caster session id、caster Zone object id、target object id、技能等级、ready_at_ms。它必须进入 Zone checkpoint/canonical root，保证 500ms 窗口内的 Zone failover 不重复或吞掉结果。

解析时重新验证：caster 仍在同 Zone、target 是存活可见 Monster、目标仍符合 Crystal `IsAttackTarget` 的本任务必要子集。对当前 Master 自己的宠物，仅在 caster attack mode 为 All（5）时允许刷新 Shock；其他模式消耗已发生但不产生结果，匹配原版延迟检查。

把概率判断抽为无副作用纯函数，例如 `electric_shock_outcome(inputs, rolls)`。生产端所有 roll 由 Zone 单写者生成/派生，客户端不得传概率值。纯函数应逐项表达原版顺序，因为后续随机数只在前一门通过时消耗。

### 2. 原子提交 Master 转移

诱惑成功应在一个 Zone command/tick 内完成：

1. 重验新 owner 容量和 Boss 容量。
2. 若有旧 owner，把 HP 改为 max HP/10，并解除旧 owner 关联。
3. 从 `native_monster_respawns` 删除目标的 respawn policy。
4. 写入新 `owner_session_id`、`owner_player_object_id`、`master_object_id`。
5. 把 disposition 改为 Friendly 并把 `hostile_to_player=false`；只改布尔值不安全，因为 checkpoint restore 会按 disposition 重算。
6. 清空 combat target、Rage、Shock、控制状态和不应跨归属继承的伤害贡献。
7. 初始化 `pet_level=0`、`pet_experience=0`、`max_pet_level=1+2*skill_level`、`tame_expires_at_ms`。
8. 同步 retained ObjectMonster 的 name、master id、shock time、颜色和 health，再统一广播。

宠物计数应扫描 `native_monsters` 中 `!dead && owner identity 匹配 && 非 intelligent creature` 的对象，而不是维护第二份易漂移的 Pets Vec。Boss 计数用 `CrystalMonsterTemplate.is_boss`。普通容量为 `skill_level+2`，Boss 上限默认 1，并留受信配置入口。

Master 不应只靠临时 `SessionId`。持久宠物快照至少携带 `account_id + character_index` 的稳定所有者身份；在线执行同时绑定当前 session id 与当前 Zone object id。重连时从稳定身份重新绑定临时字段。

### 3. 完成状态广播与 AOI 恢复

震慑成功：设置 control/shock deadline，并广播 `ObjectColourChanged(Peru)`。retained `ObjectMonster.shock_time` 应保存剩余时长，晚进入 AOI 的客户端才能还原。

难度门失败产生 Rage：增加普通怪物 `rage_until_ms`，广播 Red，并让通用选敌支持原版的野怪互攻。到期恢复正常颜色和选敌。已有 `node_ai.rs:140` 明确标注普通 Rage authoritative Zone field 仍为 OPEN，可直接作为缺口证据。

诱惑成功：广播 `ObjectHealth`（发生旧主转移时尤其必要）、`ObjectName { name: "Monster(Master)" }`，并确保后续/重入 AOI 的 retained `ObjectMonster.master_object_id` 已更新。现有玩家还需要一次能刷新 MasterObjectId 的权威对象更新；如果客户端只按原版依赖 ObjectName 展示，可保持线包最小，但 Gateway map layer必须同步 `owner_name` 和 Friendly disposition。

解除/到期：广播恢复后的 ObjectName、MasterObjectId=0/Friendly→Hostile 所需对象状态和颜色，并清除 retained 归属。若实现为释放野怪，应明确它是否重新接入 respawn；原版 TameTime 到期仅解除 Master，原对象继续存活，不恢复原 respawn 槽。

### 4. 宠物等级与战斗属性

在 `ZoneNativeMonster` 增加与 `summon_skill_level` 分离的：

- `pet_level: u8`
- `pet_experience: u32`
- `max_pet_level: u8`
- `tame_expires_at_ms: Option<u64>`
- 稳定 owner identity

这些字段用 `serde(default)` 和零值省略，避免无宠物旧 checkpoint 的 canonical bytes 无谓变化。宠物获得经验时复用 Zone 的 MonsterKillAward 归属结果：只有 owner 获得经验且宠物存活、同 Zone、在数据范围内时累加；按原版阈值升级。属性应从模板基础值加 pet level 派生，避免每次恢复重复累加。至少覆盖 HP/AC/MAC/DC；升级后发 health/name colour 所需更新。

### 5. 角色持久与 Zone 转移

新增稳定、版本化的 `TamedPetSaveRecord`，字段至少包括：monster index/name、HP、pet experience、pet level、max pet level、剩余 tame duration。写入 `CharacterSaveRecord` 的 `#[serde(default)] tamed_pets`。

Zone Leave/换图必须先产生该 owner 的宠物快照，再移除 Zone 对象。可增加 `ZoneOutbound::SaveTamedPets { session_id, pets }`；Gateway 在调用现有角色保存前把它应用到 `SimulationSession`。Join snapshot 携带恢复宠物，由目标 Zone 在 owner 附近找安全位置生成并重新绑定新的 Zone object id。

顺序必须是：捕获宠物 → 应用个人 checkpoint → 成功保存/移交 → 从旧 Zone 移除。若个人 CAS 保存失败，应沿用现有 logout recovery/fail-closed 机制，不能静默丢宠或让旧、新 Zone 各保留一份。

对于 checkpoint failover：`objects` 与 `native_monsters` 中的 master、name、disposition 必须一致；restore 后校验 owner session/稳定 identity/retained master 三者。在线 owner 不存在时宠物保持 dormant，不得作为敌对野怪行动；恢复控制面完成后重绑或执行受审计清理。

### 6. 技能熟练度回传

Zone 的 ElectricShock 解析结果应输出明确的 progression decision：第一门失败时按原版额外 1/2 roll 决定是否推进；第一门通过则推进一次。Gateway 把这个权威结果应用到个人 session 的 `advance_magic_progression` 等价入口。不要在“施法已扣 MP”时无条件升级，也不要让客户端声明结果。

## 建议写集

实现在线诱惑语义的最小核心写集：

- `apps/simulation/src/runtime/zone/runtime/electric_shock.rs`（新增）
- `apps/simulation/src/runtime/zone/runtime.rs`（模块接线、施法排队、tick 调用；由本轮唯一 runtime.rs code worker 修改）
- `apps/simulation/src/runtime/zone/types.rs`（pending、宠物归属/等级/期限字段、outbound）
- `apps/simulation/src/runtime/zone/packets.rs`（retained ObjectMonster 的 master/name/shock/color 同步）
- `apps/simulation/src/runtime/zone/runtime/checkpoint.rs`（pending/字段 canonical 与恢复一致性）
- `apps/simulation/tests/electric_shock_zone.rs`（新增，不继续堆进 runtime/tests.rs）

完成 logout/换图/重登持久所需的附加窄写集：

- `apps/simulation/src/config.rs`（`TamedPetSaveRecord` 与 `CharacterSaveRecord.tamed_pets`）
- `apps/simulation/src/runtime/save.rs`（快照/恢复）
- `apps/simulation/src/runtime/session.rs`（应用 Zone 宠物快照、join snapshot）
- `apps/simulation/src/world_runtime.rs`（窄接口）
- `apps/gateway/src/routing.rs`（处理 SaveTamedPets，保证 leave/save 顺序）
- `apps/simulation/tests/electric_shock_persistence.rs`（新增）
- `apps/gateway/src/routing/tests/electric_shock_persistence_tests.rs`（新增模块，避免继续扩大 routing.rs 内联测试）

若同一轮还对齐禁宠地图，需要扩展 Crystal map 数据生成与 `ZoneMapMetadata.no_pets`；这应独立成一个数据切片，因为当前生成清单没有该字段，不能用硬编码地图名代替。

## 必要测试

### 概率与资格（纯函数，无随机 flaky）

1. 枚举第一门 roll，验证技能 0/1/2/3 通过率分母为 4/3/2/1。
2. 第一门失败：只有 progression roll 命中时产生一次熟练度推进，绝不产生 Shock/Rage/Tame。
3. 第一门通过：总是产生一次熟练度推进；第二门一半 Shock、一半进入 Tame。
4. `target.level == player.level+2` 可诱惑，`+3` 只可能在前序分支震慑，不能诱惑。
5. `can_tame=false`、Boss cap=0/已满、普通容量已满分别阻止诱惑，但不改变前序已发生的 Shock/LevelMagic 语义。
6. 难度门边界严格验证 `roll > target.level+10`；失败时 rage roll 4/5，目标已有 Master 时不 Rage。
7. HP rate 验证 max HP 1/200/299 均以 2 为分母，300 为 6，1000 为 20；使用 max HP 而非当前 HP。

### 在线归属事务

1. 野怪诱惑成功：owner/master 三字段、Friendly disposition、hostile=false、max pet level、期限全部正确；respawn policy 被移除。
2. 转主：旧主计数减少，新主增加，HP 精确变为 maxHP/10，旧主不能再驱动宠物，伤害贡献不泄漏。
3. 容量分别验证技能 0/1/2/3 的 2/3/4/5。
4. Boss 和普通宠物容量独立验证；dead pet 不占容量，intelligent creature 不占普通诱惑容量。
5. 自己的宠物在 AttackMode.All 时刷新 Shock，在其他模式下施法可扣费但无控制结果。
6. 500ms 前目标死亡、移图、换 incarnation 或 caster 离开时 pending action安全取消，不把复用 object id 的新怪诱惑。
7. 施法时扣 MP/广播起手，结果仅在 500ms tick 后发生；重复 tick 不重复提交。

### 广播与 AOI

1. Shock/Rage/成功 Tame 分别验证 Colour、Name、Health、ObjectMonster master 字段。
2. 已在 AOI 的两个观察者收到同一归属变化；AOI 外玩家不收 delta。
3. 后进入 AOI 的玩家从 retained ObjectMonster 看到剩余 ShockTime、MasterObjectId、主人化名称和正确 disposition。
4. TameTime 到期恢复名称/颜色/敌对行为；不重新注册旧 respawn 槽。

### checkpoint、退出和跨 Zone

1. pending 500ms action checkpoint/restore 后只解析一次，并防 object-id incarnation ABA。
2. 已诱惑宠物 checkpoint/restore 后仍 Friendly、同 Master、同 HP/等级/期限；不能被 restore 的 disposition 重算重新敌对。
3. leave 清理能命中 retained master，所有观察者收到 ObjectRemove，无孤儿对象、occupancy 或 respawn 槽。
4. logout 保存后 relogin恢复；剩余 TameTime按墙钟减少；过期记录不生成。
5. 换地图在目标 Zone 只生成一份，旧 Zone 无残留；保存失败走恢复路径，不能复制或丢失。
6. CharacterSaveRecord 旧 JSON 没有 `tamed_pets` 时安全默认为空；CAS stale writer 不能覆盖新宠物状态。

### 技能与宠物升级

1. 对原版各概率分支验证 ElectricShock 熟练度推进次数。
2. 宠物经验阈值 `(level+1)*20000`、最高等级封顶和多级累计行为。
3. 升级属性从模板基线派生，checkpoint/restore 不重复叠加；HP/AC/MAC/DC 与原版增量一致。

## 不应采用的局部修补

- 只把 `control_until_ms` 改成随机：仍没有 Master、容量、广播和持久。
- 只写 `master_object_id`：AI 还看 `owner_session_id/hostile_to_player`，checkpoint restore 会重新敌对，leave 还可能因 retained 包未同步而漏清。
- 把 `summon_skill_level` 当宠物等级：会混淆“召唤技能强度”“宠物当前等级”“宠物最高等级”三个原版独立概念。
- 只发 `ObjectName`：当前观察者可能看到名称，但 Gateway map layer、晚加入 AOI、离区清理和 checkpoint 仍不知道 Master。
- 在 Gateway 或客户端决定诱惑概率：会破坏 Zone 单写权威并允许重放/客户端篡改。
- 诱惑后保留原 respawn policy：宠物死亡或恢复时会复制/复活为野怪，破坏对象生命周期和奖励幂等。

## 待运行态验证

- R4 实机尚未抓取 ElectricShock 对同一怪物多次施法的控制/颜色/归属包；源码已能确定没有 Tame 分支，但抓包可作为修复前基线。
- 原版 `NoPets` 地图数据尚未导出到 mir2-web3；需从 Crystal Server.MirDB/map info 生成，不能从源码推断具体地图集合。
- 原版服务重启后默认 `PetSave=false` 的 TameTime 是否因 `PetInfo` 二进制缺字段而重置/丢失，需单独做原版持久性实验；mir2-web3 的安全实现应保存明确的剩余期限。
- 普通诱惑宠物应受哪些现有 Zone 特殊 AI 白名单/黑名单影响，需要用至少一个普通近战怪和一个特殊 AI 怪做实机对照；`CanTame` 仍是首要权威门。
