# Three-class 66-skill acceptance matrix

Scope is the 66 `required` Active MagicInfo rows in `three-class-skill-matrix.json`: Warrior 17, Wizard 24, Taoist 25. `FastMove` is the one excluded source row. This is an acceptance ledger, not a claim that assets, source traces, automated tests, skill-page screenshots, or isolated effects equal Crystal parity.

Status at this review: the R3 ledger still reports `accepted=false` and `visualAccepted=false`; R4 has not established all-skill paired acceptance. A skill is **fully passed** only when all four gates are evidenced independently: **F** functional effect and prerequisite behavior, **A** animation/effect sequence, **U** skill UI/name/icon/page/binding, and **C** paired Crystal comparison. On the evidence supplied here, **fully passed: 0/66**. “Observed UI” means a page/row was seen; it does not close F/A/C.

Evidence shorthand: `M` = `three-class-skill-matrix.json`; `L` = `r3-live-ledger.json`; `R3` = `three-class-live-20260910.md` and its referenced R3 evidence; `R4` = same live report's R4 section and `C:/mir2-three-class-20260910/r4-visual-evidence`; `P` = `remaining-qa-groups.md`. `pending` is intentional where a complete, skill-specific paired result is absent.

## Warrior (17)

| # | Skill | F | A | U | C | Evidence version | Next gap |
|---:|---|---|---|---|---|---|---|
| 1 | Fencing | pending | pending; passive has no independent cast VFX | page row observed; paired row/binding review pending | pending | M; L; R3 Warrior pages 22–24 | Compare passive accuracy/stat behavior and original page/binding. |
| 2 | Slaying | pending; passive/probabilistic valid-melee attack overlay not live-closed | pending; no standalone cast expected, attack overlay remains open | page row observed; R3 metadata repair still needs live rerun | pending | M; L; R3 source/metadata notes | Isolate all other attack toggles, then use ordinary legal melee to verify passive attack chance/DC and the source overlay. |
| 3 | Thrusting | pending; requires armed skill plus valid attack | pending | page row observed | pending | M; L; P melee group | Test a target in the planned second-tile position and compare attack result. |
| 4 | HalfMoon | pending; requires valid melee and multiple targets | pending | page row observed | pending | M; L; P melee group | Isolate from other attack toggles and compare multi-target hit range/effect. |
| 5 | ShoulderDash | pending; corridor and collision cases absent | pending | page row observed | pending | M; L; P corridor group | Test traversable corridor and blocking target with authoritative movement correction. |
| 6 | TwinDrakeBlade | pending; preparation/target attack route not live-closed | pending | page row observed | pending | M; L; R4 preparation notes; P melee group | Capture original preparation then target attack, second damage/effect, and shared route. |
| 7 | Entrapment | pending; distance/level gates and pull/stun absent | pending | page row observed | pending | M; L; P corridor group | Use a legal live enemy and verify pull, stun, rejection, and observer state. |
| 8 | FlamingSword | pending; valid melee required | pending | page row observed | pending | M; L; P melee group | Compare armed attack overlay and target hit against Crystal. |
| 9 | LionRoar | pending; directional target result absent | pending | page row observed | pending | M; L; P direction group | Test facing, range, target effect and full animation sequence. |
| 10 | CrossHalfMoon | pending; isolated surrounding melee absent | pending | page row observed | pending | M; L; P melee group | Disable competing toggles and compare surrounding hit/effect. |
| 11 | BladeAvalanche | pending; area-depth damage absent | pending | page row observed | pending | M; L; P area group | Use multiple live targets at planned depths; verify all affected cells and damage. |
| 12 | ProtectionField | pending; self Buff effect absent | pending | page row observed | pending | M; L; P self-Buff group | Record cast, Buff layers/stats, duration/end and Crystal sequence. |
| 13 | Rage | pending; self DC change absent | pending | page row observed | pending | M; L; P self-Buff group | Capture before/after attributes and expiry without contaminating baseline. |
| 14 | CounterAttack | pending; conditional attacker-hit callback absent | pending | page row observed | pending | M; L; P counterattack group | Arm the window and let an adjacent active enemy hit; compare counterstrike. |
| 15 | SlashingBurst | pending; directional/area result absent | pending | page row observed | pending | M; L; P direction/area group | Test legal facing, targets, damage and effect timing. |
| 16 | Fury | pending; self Buff effect absent | pending | page row observed | pending | M; L; P self-Buff group | Verify source name, cast, stats, duration and Crystal visual. |
| 17 | ImmortalSkin | pending; cast was assigned but not completed live | R4 two parallel layers are implemented and covered by automated tests; live visual sequence remains pending | page row observed | pending | M; L; R4 integration/focused test notes; P self-Buff group | Run the R4 package and compare both layers, persistence and end state against Crystal. |

## Wizard (24)

| # | Skill | F | A | U | C | Evidence version | Next gap |
|---:|---|---|---|---|---|---|---|
| 18 | FireBall | partial sample: MP-9 and 20 damage observed; complete functional closure pending | partial sample: visible projectile/target explosion and trace observed; full sequence parity pending | four-page Wizard UI observed | pending | M; L; R3 Wizard pages 16/17/20/21 and quick Ctrl+F1 sample; P single-target group | Repeat with a controlled live target and capture the complete candidate/Crystal cast, projectile, impact, HP and timing comparison. |
| 19 | Repulsion | pending; R3 distant-cursor self-point gate rejected | pending | four-page UI observed | pending | M; L; R3 self-point finding; P push group | Re-test corrected self rule with adjacent pushable enemy, empty space and distant cursor. |
| 20 | ElectricShock | pending; R3 control is not proof of taming ownership | pending | four-page UI observed | pending | M; L; R3 control caveat; P tameable group | Separate shock/pause from successful Master/pet-list/name ownership and compare Crystal. |
| 21 | GreatFireBall | pending | pending | four-page UI observed | pending | M; L; R3 Wizard pages; P single-target group | Capture complete projectile/impact and HP result against Crystal. |
| 22 | HellFire | pending; directional target untested | pending | four-page UI observed | pending | M; L; P direction group | Test facing and line target with complete caster/effect sequence. |
| 23 | ThunderBolt | pending | pending | four-page UI observed | pending | M; L; P single-target group | Capture lightning, target HP and paired Crystal timing. |
| 24 | Teleport | pending; authoritative post-teleport movement not tested | pending | four-page UI observed | pending | M; L; P teleport group | Test legal map teleport, coordinate/camera consistency and subsequent movement. |
| 25 | FireBang | pending; R4 MP/ObjectMagic/ground sample does not prove damage | R4 ground explosion observed; full paired sequence pending | four-page UI observed | pending | M; L; R3 FireBang failure; R4 `02-firebang-0..6.png`; P ground group | Verify live enemy area damage, completion timing, cancellation and Crystal pairing. |
| 26 | FireWall | pending | pending | four-page UI observed | pending | M; L; P ground group | Test standable ground, entry damage, persistence and natural end. |
| 27 | Lightning | pending; directional target untested | pending | four-page UI observed | pending | M; L; P direction group | Compare direction, wall/effect duration and target result. |
| 28 | FrostCrunch | pending | pending | four-page UI observed | pending | M; L; P single-target group | Capture projectile, hit, freeze state and duration in both clients. |
| 29 | ThunderStorm | pending; area result absent | pending | four-page UI observed | pending | M; L; P nearby/undead group | Use nearby live targets and compare area damage/effect. |
| 30 | MagicShield | R4 input gate fixed, but persistence/mitigation not proven | R4 opening observed; persistent icon absent; Crystal opening/buff baseline exists | four-page UI observed; Ctrl+F1 assignment observed | pending | M; L; R3 pointer/chord failures; R4 `01-shield-0..4.png`; original `06-magicshield-sequence-0..4.png`; P self-Buff group | Lifecycle patch integrated; isolated simulation 2/2, Windows 2/2, Gateway 1/1 (shield-fix-notes.md). Rebuild main workspace, then verify persistent Buff, mitigation, expiry and paired sequence; snapshot HUD/no-tile Up remain open. |
| 31 | TurnUndead | pending; real undead target and probability absent | pending | four-page UI observed | pending | M; L; P undead group | Test live undead, rejection/probability cases and successful kill against Crystal. |
| 32 | Vampirism | pending; wounded caster and life return absent | pending | four-page UI observed | pending | M; L; P wounded-player group | Pre-damage caster, then verify target effect, damage and returned HP. |
| 33 | IceStorm | pending; group ground damage absent | pending | four-page UI observed | pending | M; L; P ground group | Test reachable ground group, area HP, persistence and cancellation. |
| 34 | FlameDisruptor | pending | pending | four-page UI observed | pending | M; L; P single-target group | Capture valid target impact/damage and any non-undead comparison. |
| 35 | Mirroring | pending; clone ownership/follow/attack absent | pending | four-page UI observed | pending | M; L; P self/pet group | Use legal pet-capacity map and adjacent space; verify clone lifecycle and attack. |
| 36 | FlameField | pending; nearby sustained damage absent | pending | four-page UI observed | pending | M; L; P nearby group | Compare player-centered field, tick damage and natural end. |
| 37 | Blizzard | pending; sustained ground result absent | pending | four-page UI observed | pending | M; L; P ground group | Capture full channel, area effect, cancellation and natural end. |
| 38 | MagicBooster | pending; stat/MP cost sequence absent | pending | four-page UI observed | pending | M; L; P self-Buff group | Test after base damage measurements and compare stat/MP changes. |
| 39 | MeteorStrike | pending; original animation exists without target damage proof | original caster/ring/meteor captured; candidate animation pending | four-page UI observed | pending | M; L; original `09-meteor-sequence-0..7.png`, `10-meteor-later.png`; P ground group | Capture candidate and Crystal target damage, persistence and cancellation side by side. |
| 40 | IceThrust | pending; shared route and multi-row target result absent | pending | four-page UI observed | pending | M; L; P shared-route group | Test planned front rows, freeze/damage, observer consistency and Crystal route. |
| 41 | StormEscape | pending; personal compatibility/shared authoritative route unresolved | pending | four-page UI observed | pending | M; L; P shared-route group | Verify legal destination, origin area effect, authoritative movement and no rollback. |

## Taoist (25)

| # | Skill | F | A | U | C | Evidence version | Next gap |
|---:|---|---|---|---|---|---|---|
| 42 | Healing | pending; wounded self/ally absent | pending | R2 Taoist page observed; displayed name issue requires R3 check | pending | M; L; R3 pages 10–13; P wounded-player group | Verify source name, target/self HP increase, cast animation and paired Crystal. |
| 43 | SpiritSword | pending; passive accuracy/DC not live-tested | passive has no independent cast VFX | R2 Taoist page observed | pending | M; L; R3 pages 10–13; P passive group | Compare passive stat behavior and page/binding; do not expect active animation. |
| 44 | Poisoning | pending; equipped green/red poison cases absent | pending | R2 Taoist page observed | pending | M; L; P poison group | Equip each poison type, test live enemy consumption/status and Crystal sequence. |
| 45 | SoulFireBall | pending; equipped amulet and legal line target absent | pending | R2 Taoist page observed | pending | M; L; P amulet/enemy group | Verify amulet equipment lookup, three-part cast/projectile/hit and damage. |
| 46 | SummonSkeleton | pending; summon capacity/map/amulet closure absent | pending | R2 Taoist page observed | pending | M; L; P summon group | Verify legal map, material, valid cell, summon lifecycle and capacity rejection. |
| 47 | Hiding | pending; equipment and cancel-on-move/hit absent | pending | R2 Taoist page observed | pending | M; L; P self-Buff group | Equip amulet, capture hide, movement cancellation and hit cancellation. |
| 48 | MassHiding | pending; friendly area target absent | pending | R2 Taoist page observed | pending | M; L; P friendly-Buff group | Use live ally in range and compare projectile, burst and ally hide state. |
| 49 | SoulShield | pending; ally Buff and equipped amulet absent | pending | R2 Taoist page observed | pending | M; L; P friendly-Buff group | Verify amulet, ally target, MAC/Buff state, persistence and Crystal visual. |
| 50 | Revelation | pending; target/probability duration absent | pending | R2 Taoist page observed | pending | M; L; P live-target group | Use live player/monster and record success, duration and repeat behavior. |
| 51 | BlessedArmour | pending; ally Buff absent | pending | R2 Taoist page observed | pending | M; L; P friendly-Buff group | Verify amulet, projectile/burst, AC change, duration and Crystal pairing. |
| 52 | EnergyRepulsor | pending; group push target absent | pending | R2 Taoist page observed | pending | M; L; P push group | Use nearby low-level enemies and legal empty cells; compare authoritative displacement. |
| 53 | TrapHexagon | pending; legal enemy and ground rejection absent | pending | R2 Taoist page observed | pending | M; L; P live-target group | Test valid enemy placement and empty-ground rejection, then compare trap effect. |
| 54 | Purification | pending; clearable abnormal/poison state absent | pending | R2 Taoist page observed | pending | M; L; P wounded/abnormal group | Create real removable status on self/ally and verify clear result. |
| 55 | MassHealing | pending; wounded ally ground target absent | pending | R2 Taoist page observed | pending | M; L; P wounded-player group | Place wounded ally in ground area and compare heal amount/effect timing. |
| 56 | Hallucination | pending; legal enemy behavior-change case absent | pending | R2 Taoist page observed | pending | M; L; P live-target group | Test eligible enemy and record success/rejection and behavior change. |
| 57 | UltimateEnhancer | pending; eligible ally/pet target absent | pending | R2 Taoist page observed | pending | M; L; P friendly/pet group | Equip amulet, target live ally or own pet, compare Buff/stat result. |
| 58 | SummonShinsu | pending; material/capacity/delayed summon absent | pending | R2 label observed as “Summon Shinsu”; R3 name check pending | pending | M; L; R3 Taoist name note; P summon group | Verify source name, material, capacity, appearance, transform and attack. |
| 59 | Reincarnation | pending; dead second-client player absent | pending | R2 Taoist page observed | pending | M; L; P death/resurrection group | Swap to resurrection charm, run accepted two-client death/revival and compare channel/time. |
| 60 | SummonHolyDeva | pending; capacity and delayed summon absent | pending | R2 Taoist page observed | pending | M; L; P summon group | Verify legal cell/material/capacity, delayed appearance and attack behavior. |
| 61 | Curse | pending; equipped amulet and enemy group absent | pending | R2 Taoist page observed | pending | M; L; P enemy-group group | Verify projectile/ground burst, enemy status and Crystal timing. |
| 62 | Plague | pending; equipped amulet/optional poison case absent | pending | R2 Taoist page observed | pending | M; L; P poison/enemy group | Test base and poison-enhanced cases with live enemy group and duration. |
| 63 | PoisonCloud | pending; five-material fixture and cloud damage absent | pending | pending | pending | M; L; P poison-cloud group | Equip required fixture after source check; verify both consumptions, cloud and persistent status. |
| 64 | EnergyShield | pending; player-only target and hit response absent | pending | R2 Taoist page observed | pending | M; L; P wounded-player group | Use live player target, verify shield, hit response/recovery and rejection of non-player target. |
| 65 | PetEnhancer | pending; own living pet target absent | pending | R2 Taoist page observed | pending | M; L; P pet group | Target own live summon, verify Buff/stat and subsequent pet attack. |
| 66 | HealingCircle | pending; ground area with wounded ally absent | pending | R2 Taoist page observed | pending | M; L; P wounded-player group | Use wounded ally in legal ground area and compare persistent healing circle. |

## Count and evidence boundary

The rows above are an explicit 66-row closure check: 17 Warrior + 24 Wizard + 25 Taoist. UI page observations are recorded where the supplied live report says they occurred, including Wizard pages 1–4, Warrior pages 1–3, and Taoist pages 1–4. Original Crystal captures exist for selected Wizard samples (MagicShield, FireBang, MeteorStrike), but they are not paired evidence for every corresponding candidate row. Therefore the independently defensible count of skills passing **F + A + U + C** is **0/66**. The next acceptance round must attach skill-specific candidate and Crystal evidence for each open row, including real target/effect state, not merely a source/resource/test result.
