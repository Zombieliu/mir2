# MagicBooster / ManaPenaltyPercent MP audit

只读审计，未修改源码、未构建、未操作客户端。核对对象是 `shield-diagnosis.md` 的 MP 段、当前 E 工作区的 Gateway/Zone/Session 路径，以及原版 Crystal 客户端的 MP 预测公式。

## 结论

当前共享玩家施法链只传基础 MP 消耗：

```text
baseCost(level) = Magic.baseCost + Magic.levelCost * skillLevel
```

`apps/simulation/src/runtime/combat.rs:3310-3333` 的 `zone_magic_attack_profile` 计算这个值；`apps/gateway/src/routing.rs:10518-10530` 写入 `ZoneNativePlayerAttackKind::Magic.mp_cost`；Gateway 后续在 `10705-10727` 将同一个值放入 `ZoneCommand::PlayerCastMagicWithItem`，成功后 `10786-10789` 再用同一个值调用个人 Session 的 `apply_zone_player_magic_spend`。Zone 各种实际分支（monster、ground、self、friendly player、summon、pet）均按收到的 `mp_cost` 检查并扣除。

MagicBooster 在 `apps/simulation/src/runtime/zone/runtime.rs:6889-6906` 已创建 stat 127 `ManaPenaltyPercent`；等级 3 的自动测试还确认该 Buff 值为 9（`runtime/tests.rs:46636-46640`）。但 `ZonePlayerCombatStats`（`runtime/zone/types.rs:188-221`）没有该字段，当前 `zone_magic_attack_profile` 也没有读取 stat 127。因此 MagicBooster 的 MC 增益可以存在，MP 代价仍按基础值计算。

原版基础消耗不是对所有技能都一条公式到底。服务端 `E:/mir2/Crystal/Server/MirObjects/HumanObject.cs:3375-3397` 的顺序是：先算基础 cost；Teleport、Blink、StormEscape 先加 stat 128 `TeleportManaPenaltyPercent`；再加 stat 127 `ManaPenaltyPercent`；最后 Plague 直接覆盖为 `MaxSC + MinSC`。客户端 `Client/MirScenes/GameScene.cs:11797-11810` 复制了基础值、stat 128、stat 127 的预测顺序，但该段客户端预测没有服务端的 Plague 覆盖。因此 Plague 需要记录为“原版服务端/客户端预测存在差异”，不能笼统宣称 66 技能共享同一 MP 公式。

对普通非特殊魔法，整数公式是：

```text
penalty = max(0, ManaPenaltyPercent)
effectiveCost = baseCost + floor(baseCost * penalty / 100)
```

因此普通 MagicShield 的 3 级基础值 `35 + 5*3 = 50`，在 6% penalty 下为 `50 + floor(50*6/100) = 53`。诊断中记录的原版 53 与此吻合；蓝色 Buff 图标本身不能证明其来源就是 MagicBooster，也不能单独证明 stat 127 的总值。Teleport/Blink/StormEscape 必须先叠加 stat 128；Plague 则以服务端最终覆盖值为准，不能套普通技能的最终结果。

## 当前链路的关键差距

1. Session 的 `player_stats()` 已经会从装备、活动 Buff 和公会 Buff 汇总 Crystal stat（`runtime/stats.rs:388-420`、`636+`），所以个人世界能得到 stat 127；但 `crystal_zone_player_combat_stats` 只投影 DC/MC/SC、护甲、抗性等字段，没有投影 127。
2. Join 时 Gateway 在 `routing.rs:8694-8702` 将 `join.combat_stats` 写入 Zone。当前搜索到的生产路径没有一个在 MagicBooster Add/Remove 后重新发送 `UpdatePlayerCombatStats`；这意味着即使新增字段，也必须处理动态 Buff 生命周期，而不能只改 join snapshot。
3. Zone 的 `ZonePlayer` 确实保存活动 Buff（`runtime/zone/types.rs:1039+`，`buffs: BTreeMap<u8, ZonePlayerBuff>`），且 MagicBooster 的 stat 127 已在其中；这是 Zone 侧验证/重算 penalty 的可用来源。若让 Zone 直接从该表重算，就必须同时保证 Gateway 的个人 Session 镜像使用相同 effectiveCost，避免 Zone MP 与 HUD/Session MP 分叉。
4. 当前 E 客户端没有发现另一个独立的本地技能 MP 预测公式；客户端主要接收 Gateway/Zone 的 `ObjectMana`/快照。原版客户端确有预测公式，必须与服务端保持同一整数舍入规则，不能以“服务器扣完后 HUD 看起来变化”替代公式对齐。

## 最小实现集合（建议顺序）

建议把“计算一次、复用一次”作为边界，避免 Zone、Gateway 和 Session 各自取整：

1. 在 `ZonePlayerCombatStats` 增加受信任的 `mana_penalty_percent: i32`，由 `crystal_zone_player_combat_stats` 从完整 `player_stats(world).get(CRYSTAL_STAT_MANA_PENALTY_PERCENT)` 填充。负值按 0 处理；字段默认 0，旧快照兼容。
2. 在 simulation 侧提供一个纯整数 helper（可放在 combat/zone 共享的现有兼容位置）：`effective_magic_mp_cost(base_cost, penalty_percent) = base + ((base as i64 * max(penalty,0) as i64) / 100)`，结果以非负 `i32` 饱和。不要使用浮点，也不要四舍五入。
3. 让 `zone_magic_attack_profile` 返回按技能类别处理后的 effectiveCost：普通技能按 stat 127；Teleport/Blink/StormEscape 按 stat 128 后再按 stat 127；Plague 按已核对的服务端覆盖语义单独处理。Gateway 因而把同一个 effectiveCost 传给 Zone，并传给 `apply_zone_player_magic_spend`；当前已分类的 magic target 分支继续复用这个值，不能只修 monster target 或只修 MagicShield。
4. 让动态 Buff 改变后刷新这份受信任 stats：MagicBooster 成功加入、刷新、到期移除，以及重连/晚加入快照都要触发一次 `UpdatePlayerCombatStats`，或由 Zone 在自己的 Buff Add/Remove 路径同步等价的 `mana_penalty_percent`。不要把客户端发送的 stat 当来源。
5. Zone 仍应在最终扣除前做 fail-closed 的有效值检查；若实现选择 Zone 根据活动 `ZonePlayer.buffs` 自己重算，则 Gateway 不应再二次套用 penalty，必须让 Gateway/session 使用 Zone 返回或明确共享的 effectiveCost。两层都算会把 53 错扣成更高值。
6. stat 128 `TeleportManaPenaltyPercent` 保持独立，并按原版顺序先于 stat 127 应用于 Teleport/Blink/StormEscape；不能把 128 合并进 127，也不能让普通技能受 128 影响。Plague 的服务端最终 cost 覆盖和客户端预测缺失必须单独保留测试/文档标记。

如果要保持更小的改动面，可暂不扩展 `ZonePlayerCombatStats`，仅让 Gateway 的 trusted `zone_magic_attack_profile` 从当前 Session 的完整 PlayerStats 得到 penalty，并在 MagicBooster Add/Remove 后重新取 profile；但这会把 Zone 的最终校验留在 Gateway，只有在现有单写入 Gateway 约束明确成立时才适用。长期更稳妥的是让 Zone 持有并验证动态 penalty，同时向 Session 镜像复用同一 effectiveCost。

## 受影响技能范围

受影响范围不是只有 Wizard 或 MagicShield，而是“角色拥有正数 stat 127 时，所有实际走 `zone_magic_attack_profile`/`PlayerCastMagic*` 的魔法施法”，同时保留特殊技能的 stat 128/Plague 分支：

- 当前 Gateway 明确分类并送入 shared magic profile 的 Wizard 行包括：FireBall、Repulsion、ElectricShock、GreatFireBall、HellFire、ThunderBolt、Teleport、FireBang、FireWall、Lightning、FrostCrunch、ThunderStorm、MagicShield、TurnUndead、Vampirism、IceStorm、FlameDisruptor、Mirroring、FlameField、Blizzard、MagicBooster、MeteorStrike。
- `IceThrust` 和 `StormEscape` 当前不在 Gateway 的 ground/self/summon 分类；targetID=0 时不能据此宣称已走 shared `zone_magic_attack_profile`，仍需审计其个人兼容路由和 MP 扣除路径。尤其原版服务端把 StormEscape 纳入 stat 128 特殊名单，不能因当前 shared 分类缺失而漏测。
- 其他 Warrior/Taoist 技能只有在实际进入 shared `PlayerCastMagic*` 路径时才套用这项修复；不应按职业硬编码排除。被动技能没有 MP cast 时不产生消耗差距。Plague 即使进入 ground 路径，也保留服务端 `MaxSC + MinSC` 覆盖与客户端预测缺口。
- MagicBooster 自身通常在施放前没有自己的 penalty，因此第一次开启应按基础成本；在 Buff 已经有效时再次施放/重施是否叠加，需按 Crystal 源码核对。后续 FireBall 等技能应使用有效 penalty。
- Teleport/Blink/StormEscape 另测 stat 128→stat 127 顺序；物品、药水回蓝和非魔法消耗不应错误套用 stat 127。

## 最小测试建议

自动测试先锁公式和传播，不需要启动客户端：

1. 纯公式表：基础值 0/1/50、penalty 0/6/9/负值/大值，确认整数 floor、非负和饱和行为；明确 `50,6 -> 53`、`50,9 -> 54`。
2. Profile：同一 3 级 MagicShield 或 FireBall，在无 penalty 与 6% penalty 下分别返回 50 与 53；确认 Gateway 发给 Zone 的值和 Session mirror 使用同值。
3. Buff 生命周期：施放 MagicBooster 前后、刷新、到期、重连/晚加入，确认 stat 127 从 0 变为活动值再回到 0，且不会重复叠加同一 Buff。
4. 分支矩阵：已分类的 monster target、ground、self、friendly player、summon、pet target 至少各一项；每项断言 MP 检查、扣除、ObjectMana 和拒绝不足 MP 均使用 effectiveCost。另为 IceThrust/StormEscape 的个人兼容路由各加一项，不把它们的通过结果计入 shared profile 测试。
5. 特殊公式回归：Teleport/Blink/StormEscape 验证 stat128→stat127；Plague 分别验证服务端覆盖值与客户端预测缺失的已知差异，并决定候选产品路径是复现差异还是补齐客户端。
6. 没有 MagicBooster 的 Warrior/Taoist 仍保持基础消耗；普通技能不受 stat128 污染。

实机最小对照应使用同一角色、同一技能等级和足够 MP：记录 MagicBooster 前后 stat/Buff、每次施法前后 MP、ObjectMana/快照、Buff 到期后的下一次消耗。先用 3 级 MagicShield 或 FireBall 做 `50 -> 53` 对照，再用一个 ground、一个 self、一个 summon/friendly 分支确认没有只修单一路径。截图中的 MP 数字需避开自然回蓝和抓帧偏移；必须以同一施法事件的前后状态或日志为准。

## 证据边界

本审计确认了代码链的基础消耗和 stat 127 缺口，也确认了 MagicBooster Buff 值的自动测试；没有声称补丁已经实现、编译或实机通过。`shield-diagnosis.md` 中的 53 是原版公式对照和待验证目标，不是当前 E 候选已通过的结果。
