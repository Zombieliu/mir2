# P4 shared pet combat and lifecycle evidence — 2026-10-08

This is bounded implementation/test evidence, not a released-client or human acceptance claim. Root owns integration, global parity documents, commits and deployment. P4 stopped code writes after the GREEN run below.

## Source and implemented boundaries

Primary references are the local Crystal source: `Server/MirObjects/PlayerObject.cs` (GainExp around 847; pet save around 300; restore around 1232; monster attack target around 4709), `MonsterObject.cs` (Die around 984–1000; PMode/CanMove/CanAttack around 634/650/1660; monster attack target around 2465; PetExp around 3501), and the complete `Monsters/VampireSpider.cs` / `SpittingToad.cs`. Trusted `Build/Server/Debug/Configs/Setup.ini` currently states `PetSave=False`, `MaxBossTames=1`, `PoisonResistWeight=10`, `PvpCanResistPoison=False`; SHA256 `787be15d6d7553d1ae64b12dfae68d23d5b93473cbb92d96332866d385fd5732`.

- Real shared Zone owner/pet/victim lives bind monster incarnation, player life and immutable online account/character/epoch. No personal-session RemotePlayer substitutes for a world actor. Generic non-owner entity combat continues rejecting owned sources.
- Owned hits revalidate life, owner mode, source-defined friends/safe protections and typed target online identity at impact. Human→pet melee, spells, range, fire-bounce, ground spells and poisoning have executable hooks. Skeleton/Shinsu normal PvE waits for an actual owner attack or monster threat; it cannot auto-attack an unrelated wild body.
- Human and pet positive damage update the common causal LastHitter. Real Death issues the internal receipt; command-batch ObjectDied is not billing authority. The receipt fixes victim/source lives, source point, legal red threshold 200, Brown and law/war decision, and one captured curse roll. Born issued proof survives owner logout/cold recovery and can ACK exactly once; old live actions lose authority on cold recovery.
- Human victim / pet attacker follows Crystal's unusual matrix: Guild allows a threatened player even with the same guild; EnemyGuild rejects a player; Group uses the victim's roster; RedBrown checks victim red ≥200 or an active victim Brown. Pet→pet Guild/EnemyGuild continue through causal target checks; its RedBrown branch instead requires the **attacker Master** red ≥200 and Brown not yet passed. Master identity, actual threat/Target, shock and source safe checks remain required.
- Human→pet real damage starts the Human's source Brown period and produces owner/observer colour updates; exact expiry produces both native colour projections. Fight/NoFight comes from trusted map metadata, with source conquest/ordinary-war handling retained.
- ElectricShock/BindingShot now reach actual owned or eligible wild bodies, with delayed impact, real shock/Rage colour, source taming limits, binding-center release and source time expiry. PoisonCloud and Poisoning create real owned DOT damage; actual actor/victim map transfer retains their global Node, while death/rejoin/cold revokes old lives.

## Saved pets, earned XP and online recall

`ZoneSavedPetSnapshot` is opaque and canonical-monster validated. Durable storage contains only monster index, HP, experience, level and maximum pet level. Object IDs, online epochs, queued actions and absolute tame deadlines cannot be supplied through that schema. Same-process Wizard warm state can retain remaining TameTime; `for_durable_storage()` strips it, matching Crystal's five durable PetInfo fields. Restore allocates fresh identities, applies source spawn action delay, and never clones a live owner's pet or revives a dead one.

`ZonePetExperienceAdmission` captures actual current owner/pet lives and same-map DataRange eligibility. `apply_earned_steps` keeps every actual final positive GainExp amount separate: Taoist canonical pets multiply the uint amount by three, uint arithmetic wraps, and each invocation advances at most one level at `(level+1)*20000`. Zero steps do nothing. Source HP/AC/MAC/DC, speeds and colours are refreshed on real growth. `saved_after_earned_steps` filters by source save policy; `PetSave=False` still saves canonical Taoist pets, but it does not persist a non-saveable class/species. `is_empty()` allows empty-pet Source admissions to remain cheap.

Root's personal GainExp producer/CAS stores the filtered canonical result with the character. P4's ordinary public Source transaction test proves failed persistence rolls back the entire character payload and live pet state, retry records the exact final earned step and mirrors once, and replay grants no second gain or mirror. This test uses an isolated SimulationConfig store, not a real player or live Postgres service.

Online map transfer prepares detached real Master metadata, drains each old Zone before destination adoption, and preserves the same online owner epoch. PMode 0/1/4 recalls across maps; 2/3 stays in the old map. NoPets freezes base pets in the old map and an allowed destination forces recall even from AttackOnly/None. True logout removes owned bodies across every Zone. Admission merges source-saveable pets across Zones while only current-map eligible pets earn; live mirror binds the current owner/life and preserves damage. Custom source AI60/61/62 self-destruct rules remain distinct from base follow/recall.

## Reachable Vampire / Toad hooks

Actual public `PlayerCastMagic` SummonToad and SummonVampire create native shared monsters with typed assigned Target, rather than dormant helpers. The exercised producer is Crystal's Archer compatibility class; this does **not** prove these two skills are available in the normal three-class classic client.

Toad uses its custom 3-second search clock, no Master follow, immediate range animation and MAC impact after `500 + distance*50` ms. Vampire uses its custom target pursuit/no idle follow, immediate MACAgility bite, positive actual damage for ushort-wrapped Master healing reserve, and strict `> +1000` / subsequent `> +500` healing clocks with at most 10 HP per pulse. Paused None/MoveOnly retains each custom species' assigned Target; Focus resumes it without inventing a new owner attack. Their source custom search behavior is separated from base Taoist PMode target clearing.

**Vampire death ordering is material:** base MonsterObject.Die excludes owned rewards and then clears Master before subclass explosion. The 3×3 burst therefore can hit its former safe Master and other real Humans/owned pets, but creates no former-Master PK, healing or new XP claim. Ordinary unowned monsters are eligible only when source Hallucination/Rage was active. A pre-existing real EXPOwner may retain its own wild kill/drop credit. Burst source/victims bind typed lives, untyped old checkpoint queues cannot gain new authority, revived victims reject the old burst, and cold-revoked Human Nodes receive zero old impact.

A trusted **fresh authoritative spawn** now clears only that new incarnation's four retired death/harvest/revive/remove projections before ObjectMonster. It repairs accepted ID reuse after actual removal; old copied target-life proof still rejects that newly spawned body. Normal metadata refresh does not acquire revive authority.

ObjectAttack/range animations and target HP/Struck/Death use their source-centered Broadcast audiences. Actual owner/party experience delivery remains independent of target AOI; the P3 real TCP/WS log 46 separately verified outside-AOI XP/Guild gain without leaking target monster packets.

## Verification and retained failures

Final isolated log: `C:/mir2-build/p4-owned-pet-86-owned-custom-mode-handoff.log`, Rust `+1.95.0`, scoped MSVC/SDK environment, target `C:/mir2-build/p4-owned-pet`.

| Selection | Passed | Ignored |
| --- | ---: | ---: |
| Library `owned_pet_` | 47 | 0 |
| Library `vampire_ai` | 6 | 0 |
| `shared_pet_pvp` | 38 | 0 |
| `shared_entity_combat` | 4 | 0 |
| `shared_monster_shinsu_ai` | 4 | 0 |
| Total distinct selected checks | 99 | 0 |

Commands: `cargo +1.95.0 test -p mir2-simulation --features test-support --lib owned_pet_ -- --test-threads=1`; same command with `vampire_ai`; then `cargo +1.95.0 test -p mir2-simulation --features test-support --test shared_pet_pvp --test shared_entity_combat --test shared_monster_shinsu_ai -- --test-threads=1`.

Numbered RED logs remain inspectable. Logs 79–81 exposed stale death-fixture assumptions and the real fresh-spawn tombstone defect; log 82 isolated passive 31-HP regen from Vampire healing; log 83 required live-vs-cold Human impact expectations rather than false output equality; log 85 exposed the generic PMode profile target-clear hook in custom AI60/61. No RED log was overwritten. Earlier log 74 was explicitly interrupted after an accidental broad library selection and is not acceptance evidence. Previous saved/recall/CAS GREEN logs 73/75/76/78 are retained.

## Remaining gates

- **SourceNPC GIVEPET / REMOVEPET / CLEARPETS shared pet outbox is unfinished.** Personal ECS pet helpers do not fulfill it. GIVEPET must create shared canonical bodies (count cap 5, PetLevel cap 7, MaxPetLevel 7); REMOVEPET kills canonical-name matches; CLEARPETS uses source DieNextTurn. NPC action order, especially GIVEPET then GIVEEXP, must include newly created pets in the same ordered Source result. No normal client may author the internal staging commands.
- SnakeTotem children have an immediate Monster Master. They must not borrow Human PK/XP authority solely from ultimate ownership; no expanded Totem completion is claimed here.
- Root gateway warm-cache/save/committed-step integration, live durable backend receipts, commit/CI, signed feed/public release and native two-account human frontend acceptance have their own gates. These 99 focused checks do not replace them.
