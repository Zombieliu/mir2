# Native six-language content terminology review

This is a curated override layer for `ru`, `hi`, `id`, `vi`, `th`, and `ar`.
It does not add a supported locale by itself and is not a full translation or native-speaker acceptance certificate.
The existing `en`, `zh-TW`, and `pt-BR` catalogs are read-only inputs.

## Source size and delivered scope

- All eight catalogs: 9385 stable keys, 8330 distinct English strings.
- English volume: 298175 Unicode characters by key; 263923 after exact text deduplication.
- Delivered: 1703 keys × six nonempty values = 10218 values.
- Curated reusable vocabulary: 402 English terms; 109 / 109 canonical skill names.
- Context-selected full item descriptions: 16.
- Manually authored complete source skill descriptions: 102 / 102 canonical skills × six languages.
- Exact legacy copies in common receive the same reviewed values: 102 keys; canonical + legacy = 204 full tooltip keys. These are not 204 distinct skills.
- Explicit common-catalog identical-text review: 296 / 296 scoped keys × six languages. The seven overlapping errors-review keys are reserved for the separate reviewer.
- Seven canonical skills have no source description; none is invented. The additional disabled FastMove legacy text is not counted as a supported canonical skill.
- NPC job + proper-name compositions: 194; numbered spell menu/prose labels: 159.
- Other 7682 keys still need the full baseline translation/review pipeline; none is assigned English fallback here.

| Catalog | Source keys | Distinct English | Unique English characters | Override keys |
| --- | ---: | ---: | ---: | ---: |
| common | 2417 | 2243 | 77552 | 512 |
| content | 3913 | 3458 | 98475 | 994 |
| npc-menus | 607 | 607 | 7904 | 24 |
| npc-prose | 1479 | 1478 | 58034 | 173 |
| game | 282 | 281 | 3436 | 0 |
| help | 124 | 124 | 18324 | 0 |
| quest | 398 | 398 | 15531 | 0 |
| shell | 165 | 165 | 5107 | 0 |

## Meaning and reuse rules

- IDs, canonical map filenames, script commands and network values remain unchanged; overrides are keyed by existing display keys only.
- Warrior / Wizard / Taoist / Assassin / Archer use one terminology set. The same skill label is reused for spellbook items, the magic catalog and exact NPC skill/level lines.
- `Lightning` is the linear lightning-beam skill (疾光電影); `ThunderBolt` is the separate sky-strike spell (雷電術). They must not collapse to one display name.
- `Fencing` raises accuracy. The Town Teleport description preserves “last saved safe zone”, not “main city”. Red poison is described as reducing defences.
- `Finish` means submit/complete the quest, `Return` means navigate back, and quest `Share` means share the quest, not give away rewards or account access.
- HP / MP / DC / MC / SC / AC / MAC remain stable stat abbreviations. A statement containing other English prose does not qualify for this exception.
- Potion sizes are standalone labels; explicitly authored potion names use grammatical gender/case where required. Do not mechanically paste a nominative size label into Russian, Hindi or Arabic prose.
- Name composition is bounded to reviewed full terms, exact canonical name aliases and known NPC-job prefixes. There is no general word-by-word translation of prose or player input.
- Actual NPC personal-name suffixes (e.g. Kyle, Peter, Jane) stay byte-identical. Unreviewed NPC prefixes are left to the baseline pipeline, never passed through as supposedly translated names.
- Russian Bichon geography uses Бичон consistently with the quest override layer; this display transliteration does not change Bichon map IDs or filenames.
- Map floor/direction codes (`1F`, `B1`, `(N)`), item size codes and monster numeric variants are retained exactly. They identify a variant and are not renumbered.
- English spelling errors such as `Cresent Slash` and `Ultimate Enchancer` are matched as existing display sources; stable keys and aliases are not rewritten.

## Common UI/context decisions

- The 296 common keys are selected explicitly by stable key. These translations do not become a generic English-word replacement for NPC prose or proper names.
- `AutoRun`, bag/hand/equipment weight, soulbound/trade restrictions, attack modes, refine/repair, item types, HP/MP limits, chat/mail controls and visible runtime status are translated for their actual UI use.
- `ui.home` / `ui.end` are chat-scroll-to-beginning/end controls (`apps/web/app/components/original-client-panels.tsx`), not names of keyboard keys. Their six values express scrolling to the start/end.
- `client.BuffEffect` / `client.ValueByOwnerPercent` receive increase/decrease, stat, numeric amount and suffix arguments (`crystal_ui/status_hud.rs`). The translations preserve that quantity relationship and do not invent an actor/owner or force a positive increase.
- Boundary whitespace, line breaks, numeric slots/formats, stat abbreviations, triple-brace item markup and the literal `/ResetConquest [ConquestID]` / `/StartConquest [ConquestID]` commands are retained. Translating help around a command does not enable or execute it.
- `server.Guild` has the English typo “Guid”; its stable key and UI context identify a guild. The translated display label does not rewrite the source key.
- Reserved for the errors reviewer, with no override emitted here: `log.realmInfo`, `client.AwakeningWithValue`, `client.CanPickupItems`, `client.HPDrainRate`, `server.SkillRemovedByGM`, `server.SpellChangedByGM`, `ui.targetDistance`.
- The 296 exact common English inputs are pinned by SHA-256: `87f003c079e77bbe3a12fff940c68476f11b0b6cba84003251a1964cdf812927`. Source changes require review.

## Identical text policy

- 21 covered keys have at least one target value exactly equal to English. Every case is checked by the generator.
- General unchanged categories are explicit proper creature names `Oma`, `Shinsu`, `Yimoogi` (and their numeric variants), or stat-only notation with unchanged placeholders.
- Common same-form words are permitted only by the exact key, locale and value below; they are not a blanket proper-name exemption. `reviewedIdenticalText()` exports the list for the final integration audit.

| Stable key | Locale | Exact value | Review reason |
| --- | --- | --- | --- |
| client.AttackMode_Guild | id | [Mode: Guild] | Mode and Guild are the reviewed Indonesian game labels for attack mode and player guild; no untranslated sentence. |
| client.GuildKey | id | Guild ({0}) | Guild is the consistent Indonesian game-community label; the shortcut argument is opaque. |
| client.Menu | id | Menu | The ordinary Indonesian word for a menu is Menu; this is a reviewed loanword, not a proper-name exemption. |
| ui.menu | id | Menu | The ordinary Indonesian word for a menu is Menu; this is a reviewed loanword, not a proper-name exemption. |
| ui.monster | id | Monster | Monster is an established Indonesian noun for a monster, not unreviewed English prose. |
| server.NpcInfo2 | ru | X : {0}, Y : {1} | Only coordinate-axis notation and opaque numeric slots; no natural-language prose. |
| server.NpcInfo2 | hi | X : {0}, Y : {1} | Only coordinate-axis notation and opaque numeric slots; no natural-language prose. |
| server.NpcInfo2 | id | X : {0}, Y : {1} | Only coordinate-axis notation and opaque numeric slots; no natural-language prose. |
| server.NpcInfo2 | vi | X : {0}, Y : {1} | Only coordinate-axis notation and opaque numeric slots; no natural-language prose. |
| server.NpcInfo2 | th | X : {0}, Y : {1} | Only coordinate-axis notation and opaque numeric slots; no natural-language prose. |
| server.NpcInfo2 | ar | X : {0}, Y : {1} | Only coordinate-axis notation and opaque numeric slots; no natural-language prose. |

- Partial proper-name tokens such as Bichon, Wooma, Zuma, GM/GT or a named NPC are intentional inside an otherwise localized name.
- Untranslated quests, item effects, NPC dialogue, menu sentences, or equipment adjectives are not proper names. Matching English is a missing translation unless separately reviewed and justified.
- Preserving player chat, player names and captured runtime parameters is mandatory and is separate from catalog translation coverage.

## Validation and integration

- Run `node packages/tooling/scripts/native-i18n-expansion-content.mjs --check`; use without `--check` to regenerate these two artifacts.
- Validation checks source-key existence, exactly six fields, nonempty values, exact placeholder multiset (including numeric formats), retained names/codes, all canonical skill names, and classified identical text.
- Every skill tooltip also checks literal numbers/percentages/dimensions and body stat-token counts. Required materials, casting modes, per-attack MP and cooldown lines are read from the pinned source, not inferred from a skill name.
- The 205 canonical/legacy English description inputs are pinned by SHA-256: `b2dced6e2bf8446e0e5c68a65859d162e668a0d9ec47d4e07a19826fb7c5ef3c`. Source changes stop regeneration until their meaning is reviewed.
- Self-tests compare all 102 legacy/canonical pairs in six languages and exercise distinct casting modes, no invented MP, required item combinations, cooldowns and critical effect/trigger clauses.
- Common self-tests require all 296 explicit keys, preserve all boundary whitespace/numbers/slots, verify opaque command/markup tokens, reject changing source numerals, and reject reuse of a same-form exemption at a different key/locale. They do not certify native-speaker fluency.
- The module exports `terminology()` for a baseline translator/glossary. Prefer stable-key overrides after translation; never globally replace terminology inside placeholders, links, chat or names.
- This layer owns only common/content/NPC entries. UI-worker shell/quest/game/help overrides must be merged by stable key with conflict reporting, not silently overwritten.
- Full-locale gates still need all remaining strings, source/parameter parity, Arabic shaping and bidi, Devanagari/Thai shaping, font fallback, clipping/wrapping and actual native screenshots.
- A licensed MT baseline is a draft requiring gameplay/context review. Nonempty fields or key counts alone do not establish a complete playable language.

## Skill source ambiguities and preserved limits

The six-language bodies are manually authored from the full English text and checked against Traditional context. This is source fidelity, not an independent certification that every legacy tooltip matches combat implementation. No gameplay code or original catalog is changed here.

| Source | Evidence and treatment |
| --- | --- |
| No source description | BattleCry, BindingShot, CatTongue, FireBounce, MentalState, MeteorShower and Portal remain without invented description keys. |
| Blizzard | English and Traditional describe body protection, strength/duration by skill level and waiting before reuse, effectively the ProtectionField effect. All those clauses are retained under the source channelling label; the apparent copy error needs source-author resolution. |
| CounterAttack | English says AC and AMC, while Traditional describes physical/magic defence. AMC is preserved verbatim and flagged; the translation does not silently assert it is the MAC stat. |
| PoisonCloud | Required-items line says GreenPoison; effect prose says to throw an amulet. Both are retained. No additional required material is guessed. |
| NapalmShot / OneWithNature | The source says “5×5 radius”, mixing dimensions and radius. Translation says an area of 5×5 and preserves the target/caster centre and all-target scope. This does not certify a tile-radius formula. |
| Trap | Source literal 60-second cooldown is retained. The generated magic manifest has delayBase 60000 and delayReduction 15000; the legacy prose does not explain any level-dependent reduction. Do not interpret the translation as a newly fixed cooldown rule. |
| FastMove | client.FastMoveSkillDescription is the only legacy tooltip without a matching canonical description. Its “rooted skills” meaning is unclear. The magic-manifest test explicitly excludes this commented-out placeholder; it receives no invented gameplay description in this layer. |
| Existing Traditional defects | FireBall has a trailing ThunderboltSkillDescription token and ThunderBolt retains English prose. Neither defect is copied into the new six-language text. |
| Formatting/source typos | Lightning “steak” is rendered as a lightning beam using the unambiguous context; IceStorm “Instant Castin” means instant casting; repeated FatalSword “Passive Skill” is rendered once. CrescentSlash has no casting-mode line, so none is fabricated. |
| Material/scope distinctions | BlessedArmour defence and SoulShield magic defence remain distinct; Curse/Plague require Amulet + Poison; PoisonCloud requires green poison and Poisoning powder. SummonSkeleton keeps the source AOE assertion; SummonShinsu keeps the English dog description rather than inventing extra pet abilities. |

Names are linked to the existing stable skill IDs. Warrior/archer wording is used only where the source identifies that class; no new profession eligibility, damage coefficient, duration, MP number or pet behaviour is inferred. The current/next-level and MP slots stay {0}/{1}/{2}.

## Retained first-draft findings

The first Arabic/Hindi Argos drafts were sampled from `C:/mir2-build/i18n-expansion-drafts/{ar,hi}.json` before integration. They were not accepted as a playable locale.

| Source / context | Observed first-draft error | Override action |
| --- | --- | --- |
| Fencing tooltip | Arabic “clapping”; Hindi “erecting a fence” | Canonical fencing name and complete accuracy tooltip |
| Lightning tooltip source typo “steak” | Arabic/Hindi translated a beef steak | Explicit lightning-beam effect; no source gameplay change |
| Poisoning tooltip | Arabic interpreted HP as blood pressure | Preserve HP, distinguish green HP loss from red defence reduction |
| Red Poison item effect | Arabic rendered reduces defence as producing defence | Explicit reduction of defences |
| Town Teleport | Arabic safe-zone became a safe/strongbox; Hindi retained English | Last saved safe zone retained in all six languages |
| Not Enough Mana to cast | Hindi draft lost the insufficient-mana condition | Explicit negative feedback in all six languages |
| Quest-share invitation | Hindi dropped sender and the invitation, leaving only acceptance question | Full invitation with the unchanged `{0}` sender slot |
| Core casting/tooltips | Arabic retained English cast/mana lines and lost level slots | Full core tooltips, required items, MP costs and exact slot parity |

This sample does not certify the unsampled descriptions or the other four draft languages. Remaining prose needs gameplay review; the draft `problems` list must remain a release blocker until resolved and revalidated.
