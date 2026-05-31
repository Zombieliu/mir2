# Crystal-Exact Implementation Blueprints (Item systems + Conquest)

The Crystal C# reference (`Suprcode/Crystal`) is now vendored as the `Crystal/`
submodule (`git submodule update --init`). This doc pins the **exact** Crystal
semantics for the remaining item-system and conquest work, so the hard part —
discovering the real algorithms/Settings — is done and implementation is
mechanical. Each section lists the Crystal source, the algorithm, the Rust-model
gap, the steps, and an honest completion + effort estimate.

> Tests/regression discipline: every increment keeps `mir2-simulation` lib at
> `837 pass / 70 pre-existing env failures` (empty-data failures unchanged) and
> `cargo check --workspace` green.

---

## 1. Weapon Refine — ✅ DONE (Crystal-exact)

- Crystal: `Server/MirObjects/PlayerObject.cs::RefineItem/CheckRefine`,
  `Server/Settings.cs` (`Refine*`).
- Rust: `apps/simulation/src/runtime/packets.rs` (`refine_*`),
  `Stage5RefineState` in `config.rs`.
- Implemented exactly: Settings constants (BaseChance 20, Increase 1, Crit 10/×2,
  WepStatReduce 6, ItemStatReduce 15, ore `BlackIronOre`), the full success-chance
  formula (`itemSuccess + oreSuccess + luckSuccess + base − addedStatPenalty`),
  ore-required, strictly-highest `RefinedValue`, crit, and **smash-on-fail**.
- Residual (~5%): per-instance ingredient durability/added-stats (the shared
  refine slots keep only item identity, so templates are read at full durability);
  the timed `CollectRefine` step is collapsed into the deposit→refine→check flow.

---

## 2. Recipe Crafting — ⬜ ~10% → blueprint ready (≈1–2 days)

- Crystal: `Server/MirObjects/NPC/NPCScript.cs::Craft` (lines ~1242–1449),
  `Server/MirDatabase/RecipeInfo.cs`, `ClientPackets.CraftItem`,
  `ServerPackets.CraftItem`/`NPCGoods(PanelType.Craft)`.
- Rust gap: `stage5_craft` is a 1-ore stub; `crystal_recipe_packet_manifest.json`
  has 79 recipes but is never consumed. `ClientPacket::CraftItem` flow is partial.
- Exact algorithm (atomic — validate fully, then consume, then roll):
  1. Find recipe by the recipe item's UniqueID; reject if `count==0` or `> StackSize`.
  2. Gold: reject if `Account.Gold < recipe.Gold * count`.
  3. **Tools** (NOT consumed): each tool slot must have `floor(CurrentDura/1000) >= count`.
  4. **Ingredients**: per ingredient, `Count*count` must be present in a unique
     slot; if `ingredient.CurrentDura < ingredient.MaxDura`, the inventory item's
     `CurrentDura` must meet `ingredient.CurrentDura`.
  5. All tool+ingredient slots must be matched (`usedSlots.Count ==
     tools+ingredients`), and `CanGainItem(output)` must hold.
  6. **Consume**: damage each tool by `count*1000` durability (Crystal
     `DamageItem`); delete `Count*count` of each ingredient; deduct gold.
  7. **Roll**: `Random(0,100) < recipe.Chance + Stats[CraftRatePercent]` →
     `GainItem(output × recipe.Item.Count*count)`; else fail message. Resources
     are consumed regardless of the roll.
  - `CanCraft` filters the NPC goods list by level/gender/class/flags/quests.
- Rust steps: decode the recipe manifest into a `RecipeInfo` table (ingredients
  with counts + tools + gold + chance + output); implement `CraftItem` in
  `packets.rs` using `InventoryResource` (durability is already on `ItemState`);
  deterministic roll; honest unit tests for validate/consume/roll/grant.

---

## 3. Item Socketing + Gems — ⬜ ~35% → blueprint ready (≈3–5 days, needs model change)

- Crystal: `PlayerObject.cs::EquipSlotItem` (insert socket item),
  `RemoveSlotItem` (extract), `CombineItem` shape 7 (add socket), shapes 3/4
  (apply gem → `AddedStats`); `HumanObject.cs::RefreshSocketStats` (gem stats
  aggregate via `Stats.Add(temp.AddedStats)`); `ValidGemForItem`; packets
  `CombineItem`/`RemoveSlotItem`/`EquipSlotItem`/`ItemSlotSizeChanged`/`ItemUpgraded`.
- Rust gap (structural): `ItemState` has a `socket_slots: u8` **count** but no
  `Slots: Vec<Option<ItemState>>` array holding socketed items, and no `Stats`
  dictionary / `GetTotal`. Socket-create exists; gem *insertion* is a no-op.
- Steps: (a) add `slots: Vec<Option<ItemState>>` to `ItemState` (serde-default;
  ~36 literal sites) + `SetSlotSize`; (b) `EquipSlotItem`/`RemoveSlotItem` with
  the exact validation (type match via `ValidGemForItem`, capacity =
  `RandomStats.SlotMaxStat`, cursed/wedding/soulbound checks); (c) fold socketed
  items' stats into the wearer's totals in the equipment-stat aggregation
  (mirroring `RefreshSocketStats`); (d) gem-apply success: `successchance` from
  `Reflect`×(stat or gem count), `CriticalRate`, `GemRatePercent`; (e) packets.
- The `Slots[]` array + stat-aggregation change is the bulk; once present, the
  rest is mechanical.

---

## 4. Conquest / Castle Siege — ⬜ ~30% → blueprint ready (≈2–4 weeks, large)

- Crystal: `Server/MirObjects/ConquestObject.cs` (scheduling + 4 game modes +
  `TakeConquest`), `MirDatabase/ConquestInfo.cs`/`ConquestGuildInfo.cs`,
  `MirObjects/Monsters/ConquestArcher.cs` (AI 80), `Gate.cs`/`CastleGate.cs`
  (AI 81 / siege AI 72), `Wall.cs` (AI 82), flag NPCs; `Envir.cs` per-tick
  `Conquests[i].Process()`; tax in `NPCScript.cs::PriceRate`.
- Rust gap: `Stage5ConquestState` is owner/war-name/rate/gold/guard/wall/gate
  **bookkeeping only** + NPC command surface; no scheduler, no siege combat, no
  game modes, no tax collection, no protocol, conquest teleports filtered out
  (`map.rs:300`).
- Sub-systems (each independently implementable):
  1. **Scheduler** (`AutoSchedule`, per minute): war window `[StartHour*60,
     +WarLength)` AND `CheckDay()`; `Auto` starts in-window, `Request` needs
     `AttackerID != -1`, `Forced` starts via NPC and ends at `WarEndTime`.
  2. **Game modes** & win → `TakeConquest`: CapturePalace (reach palace),
     KingOfHill (first to 18 hold-points), Classic (last guild with living
     players on palace map), ControlPoints (most flags at war end).
  3. **Siege objects** as zone monsters: Archer (AI 80, FindTarget spiral,
     0 damage to owner guild / when no war), Gate (AI 81, HP→`GetDamageLevel`
     direction states, open/close, auto-open for owner, repair), Wall (AI 82,
     4 damage levels), Siege (AI 72), Flag NPC (guild image/colour, `ChangeOwner`).
  4. **Taxes** (`PriceRate`): non-owner pays `(rate×NPCRate)+rate`; markup →
     `GuildInfo.GoldStorage`; owner pays base; withdraw → guild gold.
  5. **Owner benefits**: gate auto-open, siege-object damage immunity during war,
     tax exemption, guild name `[Conquest]` tag.
  6. **Packets** + re-enable conquest-index teleports gated on war state.
  - Hardest: the win/ownership algorithm per game mode and siege-object combat;
    both depend on a real guild model (Rust has `stage5_systems.guild`) and on
    treating siege objects as authoritative zone monsters.

---

## Honest status

| System | Before | Now | Path to production-grade |
|---|---|---|---|
| Refine | ~10% (stub) | **~95% (Crystal-exact)** | per-instance dura + timed collect |
| Recipe | ~10% | blueprint | ≈1–2 days (mechanical) |
| Socket | ~35% | blueprint | ≈3–5 days (item-model change) |
| Conquest | ~30% | blueprint | ≈2–4 weeks (large subsystem) |

The Crystal reference being vendored + these blueprints means the remaining work
is spec-complete, not research. Refine is genuinely done to Crystal-exact; the
rest is bounded, prioritized implementation.
