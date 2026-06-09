# In-Game GM Commands (`@`) — Reference

A 1:1 port of Crystal's GM chat commands
(`Crystal/Server/MirObjects/PlayerObject.cs:2152-4156`). Implemented in the
simulation at `apps/simulation/src/runtime/gm_commands.rs`.

> Players type `@COMMAND args` in the chat box. Every `@`-prefixed line is consumed
> as a command attempt and is **never echoed to public chat** (exactly as Crystal
> does), so command existence never leaks. A GM-gated command typed by a non-GM is
> silently ignored.

## Becoming a GM

Each command carries Crystal's own permission gate. Satisfy one of:

| Path | How |
|---|---|
| **Account GM rank** | The character's account `gm_level > 0` (read at `StartGame`). Primary path on the web port — set `gm_level` on the account record (JSON store `.mir2-data/accounts.json`, or the Postgres backend). |
| **`MIR2_GM_ACCOUNTS`** | Comma-separated account allowlist, e.g. `MIR2_GM_ACCOUNTS=demo`. Grants GM for the session at login **without mutating the stored record** (case-insensitive). The zero-config way to make the demo account a GM locally. |
| **`@LOGIN` password** | Set `MIR2_GM_PASSWORD=<secret>`, then in-game type `@LOGIN` and, on the prompt, the password on the **next** chat line. A match grants GM for the session. With no `MIR2_GM_PASSWORD` set, `@LOGIN` can never succeed. |
| **TestServer flag** | When the server runs as a test server, the `IsGM \|\| TestServer` tier is open to any player. |

> **Make the `demo` account a GM, the easy way:** start the server with
> `MIR2_GM_ACCOUNTS=demo` in the environment and log in as `demo` — `@`-commands
> work immediately, and nothing is written to the account store.

Permission tiers used below: **GM** = requires GM rank · **GM/Test** =
`IsGM || TestServer` · **Any** = ungated · **Feature** = a feature/state gate
(e.g. target opt-in, guild rank).

## Authentication & toggles

| Command | Effect | Tier |
|---|---|---|
| `@LOGIN` | Arm the GM-password prompt (next chat line is the password). | Any |
| `@SUPERMAN` | Toggle invincibility (`GMNeverDie`) — takes no damage and cannot die. | GM/Test |
| `@GAMEMASTER` | Toggle GameMaster mode flag. | GM/Test |
| `@OBSERVER` | Toggle Observer mode flag. | GM |
| `@ALLOWTRADE` | Toggle whether you accept trades. | Any |
| `@ALLOWGUILD` | Toggle guild-invite acceptance. | Any |
| `@ALLOWOBSERVE` | Toggle whether others may observe you (`S.AllowObserve`). | Any |
| `@ENABLEGROUPRECALL` | Toggle group-recall permission. | Any |

## Character & economy

| Command | Effect | Tier |
|---|---|---|
| `@LEVEL <1-255>` | Set your level (restores HP/MP). `@LEVEL <player> <lvl>` targets another. | GM/Test |
| `@GIVEGOLD <amount>` | Add gold (alias `@GOLD`, `@SETGOLD`). `@GIVEGOLD <player> <amt>` targets another. | GM/Test |
| `@GIVECREDIT <amount>` | Add account credit. | GM/Test |
| `@GIVEPEARLS <amount>` | Add intelligent-creature pearls. | GM/Test |
| `@ADJUSTPKPOINT <n>` | Set PK points. | GM/Test |
| `@DIE` | Die immediately (ignores `GMNeverDie`, per Crystal). | Any |
| `@REVIVE` | Revive to full HP/MP. | GM |
| `@CHANGECLASS <class>` | `Warrior`/`Wizard`/`Taoist`/`Assassin`/`Archer`. | GM/Test |
| `@CHANGEGENDER` | Flip gender. | GM/Test |
| `@HAIR [0-8]` | Set hair (no arg = random). | GM/Test |
| `@DECO <image>` | Spawn a decoration object at your location. | GM/Test |
| `@SETLIGHT <n>` | Set personal light radius (`S.PlayerUpdate`). | GM |

## Items, inventory & storage

| Command | Effect | Tier |
|---|---|---|
| `@MAKE <name\|index> [count]` | Create an item, e.g. `@MAKE WoodenSword 5` or `@MAKE 1`. | GM/Test |
| `@CLEARBAG` | Delete every inventory item. | GM/Test |
| `@ADDSTORAGE` | Expand storage (gold fee). | Any |
| `@ADDINVENTORY` | Expand inventory (gold fee). | Any |
| `@AWAKENING <ItemType> <AwakeType>` | Awaken equipped items of a type. Weapon→`DC`/`MC`/`SC`, Helmet→`AC`/`MAC`, Armour→`HPMP`. 70% success. | GM/Test |
| `@REMOVEAWAKENING <ItemType>` | Remove one awaken level from equipped items of a type. | GM/Test |

## Skills

| Command | Effect | Tier |
|---|---|---|
| `@GIVESKILL <spell> <0-3>` | Learn / set a spell level, e.g. `@GIVESKILL Fireball 3`. `@GIVESKILL <player> <spell> <lvl>` targets another. | GM/Test |
| `@DELETESKILL <spell>` | Remove a spell. `@DELETESKILL <player> <spell>` targets another. | GM |

## Monsters

| Command | Effect | Tier |
|---|---|---|
| `@MOB <name> [count] [spread]` | Spawn hostile monsters near you, e.g. `@MOB SnowWolf 5`. | GM/Test |
| `@RECALLMOB <name> [count≤50] [petLvl 0-7]` | Spawn pets that follow you. | GM/Test |
| `@KILL` | Kill the monsters in the cell in front of you (no-arg form). | GM |
| `@CLEARMOB [mapfile]` | Kill all monsters on the (current) map. | GM |

> Conquest objects (siege gates/walls/archers) cannot be spawned, matching Crystal.

## Movement & maps

| Command | Effect | Tier |
|---|---|---|
| `@MOVE <x> <y>` | Teleport within the current map. | GM/Test* |
| `@MAPMOVE <mapfile> [inst] [x] [y]` | Move to another map (single-session: current map only). | GM/Test |
| `@MAP` | Show current map title + file. | Any |

\* `@MOVE` also accepts a Teleport special item per Crystal.

## Quests & story flags

| Command | Effect | Tier |
|---|---|---|
| `@SETQUEST <id> <state>` | `1` = mark complete, `0` = cancel. | GM/Test |
| `@CLEARQUESTS` | Clear active quests. | GM/Test |
| `@SETFLAG <index>` | Toggle a story flag. | GM/Test |
| `@LISTFLAGS` | List set flags. | GM/Test |
| `@CLEARFLAGS` | Clear all flags. | GM/Test |

## Buffs, hero, mount & transform

| Command | Effect | Tier |
|---|---|---|
| `@CLEARBUFFS` | Remove all buffs. | Any |
| `@TOGGLETRANSFORM` | Pause/resume the active Transform buff (frozen against expiry while paused). | Any |
| `@RIDE` | Mount/dismount the equipped mount. | Any |
| `@SUMMONHERO` | Summon your hero. | Any |
| `@LEVELHERO <level>` | Set hero level. | GM/Test |

## Informational & misc

| Command | Effect | Tier |
|---|---|---|
| `@TIME` | Show the server time. | Any |
| `@ROLL` | Roll a 1-5 die to your group. | Any |
| `@INFO [player]` | Inspect the object in front of you (or self). | GM/Test |
| `@SETTIMER <key> <seconds> <type>` | Show a named client countdown (`S.SetTimer`). | Any |

## Server administration

| Command | Effect | Tier |
|---|---|---|
| `@RELOADDROPS` | Reload drop tables (acknowledgement only). | GM |
| `@RELOADNPCS` | Reload NPC scripts (acknowledgement only). | GM |
| `@CLEARIPBLOCKS` | Clear IP blocks. | GM |
| `@TRIGGER <key> [player]` | Fire an NPC trigger script. | GM |

## Single-session behaviour

The simulation runs one live character per world, with no other online players,
guilds, or conquests. The cross-entity commands therefore resolve to Crystal's
own responses — which is the correct 1:1 result for an empty world:

- **Players not found:** `@KILL <name>`, `@RECALL`, `@GOTO`, `@FIND`,
  `@GIVEGOLD/@GIVECREDIT/@GIVEPEARLS/@LEVEL/@ADJUSTPKPOINT <player> …`,
  `@RECALLMEMBER`, `@BACKUPPLAYER`, `@ARCHIVEPLAYER`, `@LOADPLAYER`,
  `@RESTOREPLAYER` → *"player not found / is not online"*.
- **No guild / conquest:** `@LEAVEGUILD` (silent), `@CREATEGUILD`, `@STARTWAR`,
  `@STARTCONQUEST`, `@RESETCONQUEST` → *"not in a guild / need a guild"*;
  `@GATES`, `@CHANGEFLAG`, `@CHANGEFLAGCOLOUR` → *"no access"*.
- **Not married:** `@RECALLLOVER` → *"you're not married"*.

These commands are still parsed and gated exactly as Crystal does; only the
target population is empty.
